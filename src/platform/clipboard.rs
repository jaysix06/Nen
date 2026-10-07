use anyhow::{Context, Result, ensure};
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{DataExchange::*, Memory::*},
};

struct ClipboardGuard;
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}

/// GPUI handles encoded images and copied files. Also accept Windows bitmap
/// payloads, including DIBV5 from screenshot tools and drawing applications.
pub fn clipboard_bitmap() -> Result<Option<Vec<u8>>> {
    if unsafe { OpenClipboard(None) }.is_err() {
        return Ok(None);
    }
    let _clipboard = ClipboardGuard;
    for format in [17, 8] {
        // CF_DIBV5, CF_DIB
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
            continue;
        };
        let memory = HGLOBAL(handle.0);
        let length = unsafe { GlobalSize(memory) };
        ensure!(
            length as u64 + 14 <= crate::note_images::MAX_BYTES,
            "Choose an image smaller than 20 MB"
        );
        let pointer = unsafe { GlobalLock(memory) };
        if pointer.is_null() {
            continue;
        }
        let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), length).to_vec() };
        unsafe {
            let _ = GlobalUnlock(memory);
        }
        return Ok(Some(
            dib_to_bmp(&bytes).context("Couldn't read the clipboard image")?,
        ));
    }
    Ok(None)
}

fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 || dib.len() as u64 + 14 > crate::note_images::MAX_BYTES {
        return None;
    }
    let header = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if !matches!(header, 40 | 52 | 56 | 108 | 124) || header > dib.len() {
        return None;
    }
    let bits = u16::from_le_bytes(dib[14..16].try_into().ok()?);
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let colors = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    let palette = if bits <= 8 {
        let maximum = 1usize.checked_shl(bits.into())?;
        if colors > maximum {
            return None;
        }
        if colors == 0 { maximum } else { colors }
    } else {
        colors
    };
    // V4/V5 headers already contain bit masks. Only the 40-byte header
    // stores BI_BITFIELDS/BI_ALPHABITFIELDS masks after the header.
    let masks = if header == 40 {
        match compression {
            3 => 12,
            6 => 16,
            _ => 0,
        }
    } else {
        0
    };
    let pixels = header
        .checked_add(palette.checked_mul(4)?)?
        .checked_add(masks)?;
    if pixels >= dib.len() {
        return None;
    }
    let mut bmp = Vec::with_capacity(dib.len() + 14);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&((dib.len() + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&((pixels + 14) as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_bitmaps_with_v5_embedded_masks_decode_without_pixel_offset_errors() {
        for header in [40usize, 124] {
            let mut dib = vec![0; header + 16];
            dib[0..4].copy_from_slice(&(header as u32).to_le_bytes());
            dib[4..8].copy_from_slice(&2i32.to_le_bytes());
            dib[8..12].copy_from_slice(&2i32.to_le_bytes());
            dib[12..14].copy_from_slice(&1u16.to_le_bytes());
            dib[14..16].copy_from_slice(&32u16.to_le_bytes());
            if header == 124 {
                dib[16..20].copy_from_slice(&3u32.to_le_bytes());
                for (offset, mask) in [
                    (40, 0x00ff0000u32),
                    (44, 0x0000ff00),
                    (48, 0x000000ff),
                    (52, 0xff000000),
                ] {
                    dib[offset..offset + 4].copy_from_slice(&mask.to_le_bytes());
                }
            }
            for pixel in dib[header..].as_chunks_mut::<4>().0 {
                pixel.copy_from_slice(&[160, 80, 10, 255]);
            }
            let bmp = dib_to_bmp(&dib).expect("bitmap");
            let image = image::load_from_memory(&bmp).expect("decode").to_rgba8();
            assert_eq!(image.dimensions(), (2, 2));
            assert_eq!(image.get_pixel(0, 0).0, [10, 80, 160, 255]);
        }
        assert!(dib_to_bmp(&[0; 39]).is_none());
    }
}
