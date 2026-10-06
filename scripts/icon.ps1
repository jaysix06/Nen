$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (-not ('NenIconCrop' -as [type])) {
    Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @'
using System;
using System.Drawing;
public static class NenIconCrop {
    public static Rectangle Bounds(Bitmap bitmap) {
        int left = bitmap.Width, top = bitmap.Height, right = -1, bottom = -1;
        for (int y = 0; y < bitmap.Height; y++) {
            for (int x = 0; x < bitmap.Width; x++) {
                // Ignore barely visible stray pixels outside the rounded tile.
                if (bitmap.GetPixel(x, y).A < 16) continue;
                left = Math.Min(left, x); top = Math.Min(top, y);
                right = Math.Max(right, x); bottom = Math.Max(bottom, y);
            }
        }
        if (right < left) throw new InvalidOperationException("Icon artwork is empty.");
        int side = Math.Max(right - left + 1, bottom - top + 1);
        return new Rectangle(left - (side - (right - left + 1)) / 2,
            top - (side - (bottom - top + 1)) / 2, side, side);
    }
}
'@
}
$root = Split-Path $PSScriptRoot -Parent
$source = [Drawing.Bitmap]::FromFile((Join-Path $root 'assets/nen-source.png'))
$frames = @()
try {
    $crop = [NenIconCrop]::Bounds($source)
    Write-Output "Icon crop: $($crop.X), $($crop.Y), $($crop.Width) x $($crop.Height)"
    foreach ($size in @(16,24,32,48,64,128,256)) {
        $bitmap = New-Object Drawing.Bitmap($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        $stream = New-Object IO.MemoryStream
        try {
            $graphics.CompositingMode = [Drawing.Drawing2D.CompositingMode]::SourceCopy
            $graphics.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $graphics.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $graphics.DrawImage($source, (New-Object Drawing.Rectangle(0,0,$size,$size)), $crop, [Drawing.GraphicsUnit]::Pixel)
            $bitmap.Save($stream, [Drawing.Imaging.ImageFormat]::Png)
            $bytes = $stream.ToArray()
            $frames += [PSCustomObject]@{ Size = $size; Bytes = $bytes }
            if ($size -eq 256) { [IO.File]::WriteAllBytes((Join-Path $root 'assets/nen.png'), $bytes) }
            if ($size -eq 32) { [IO.File]::WriteAllBytes((Join-Path $root 'assets/nen-tray.png'), $bytes) }
        } finally {
            $stream.Dispose()
            $graphics.Dispose()
            $bitmap.Dispose()
        }
    }
    $file = [IO.File]::Create((Join-Path $root 'assets/nen.ico'))
    $writer = New-Object IO.BinaryWriter($file)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$frames.Count)
        $offset = 6 + 16 * $frames.Count
        foreach ($frame in $frames) {
            $dimension = if ($frame.Size -eq 256) { 0 } else { $frame.Size }
            $writer.Write([byte]$dimension)
            $writer.Write([byte]$dimension)
            $writer.Write([byte]0)
            $writer.Write([byte]0)
            $writer.Write([uint16]1)
            $writer.Write([uint16]32)
            $writer.Write([uint32]$frame.Bytes.Length)
            $writer.Write([uint32]$offset)
            $offset += $frame.Bytes.Length
        }
        foreach ($frame in $frames) { $writer.Write([byte[]]$frame.Bytes) }
    } finally {
        $writer.Dispose()
        $file.Dispose()
    }
} finally { $source.Dispose() }
