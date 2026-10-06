//! Unpackaged desktop toast activation, using Windows' COM server contract.
use super::windows::{APP_ID, PlatformEvent};
use std::ffi::c_void;
use windows::{
    Win32::{Foundation::*, System::Com::*, UI::Notifications::*},
    core::*,
};

const ACTIVATOR: GUID = GUID::from_u128(0x97ea4456_9552_4e55_bd74_57ac0e379f48);

#[implement(INotificationActivationCallback)]
struct Activation {
    sender: async_channel::Sender<PlatformEvent>,
}
impl INotificationActivationCallback_Impl for Activation_Impl {
    fn Activate(
        &self,
        app: &PCWSTR,
        arguments: &PCWSTR,
        _data: *const NOTIFICATION_USER_INPUT_DATA,
        _count: u32,
    ) -> Result<()> {
        // COM guarantees these two strings remain valid for the duration of this call.
        if !app.is_null() && !arguments.is_null() && unsafe { app.to_string()? } == APP_ID {
            let _ = self.sender.try_send(PlatformEvent::Notification(unsafe {
                arguments.to_string()?
            }));
        }
        Ok(())
    }
}

#[implement(IClassFactory)]
struct Factory {
    sender: async_channel::Sender<PlatformEvent>,
}
impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        object: *mut *mut c_void,
    ) -> Result<()> {
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        if iid.is_null() || object.is_null() {
            return Err(E_POINTER.into());
        }
        let activation: INotificationActivationCallback = Activation {
            sender: self.sender.clone(),
        }
        .into();
        unsafe { activation.query(iid, object).ok() }
    }
    fn LockServer(&self, _lock: BOOL) -> Result<()> {
        Ok(())
    }
}

pub struct Registration(u32);
impl Registration {
    pub fn new(sender: async_channel::Sender<PlatformEvent>) -> anyhow::Result<Self> {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};
        let clsid = "{97EA4456-9552-4E55-BD74-57AC0E379F48}";
        let root = RegKey::predef(HKEY_CURRENT_USER);
        root.create_subkey(format!("Software\\Classes\\AppUserModelId\\{APP_ID}"))?
            .0
            .set_value("CustomActivator", &clsid)?;
        let executable = std::env::current_exe()?;
        root.create_subkey(format!("Software\\Classes\\CLSID\\{clsid}\\LocalServer32"))?
            .0
            .set_value(
                "",
                &format!("\"{}\" --toast-activated", executable.display()),
            )?;
        let factory: IClassFactory = Factory { sender }.into();
        let token = unsafe {
            CoRegisterClassObject(
                &ACTIVATOR,
                &factory,
                CLSCTX_LOCAL_SERVER,
                REGCLS_MULTIPLEUSE,
            )
        }?;
        Ok(Self(token))
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = unsafe { CoRevokeClassObject(self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn com_activation_dispatches_arguments_and_checks_identity() {
        let (sender, receiver) = async_channel::unbounded();
        let callback: INotificationActivationCallback = Activation { sender }.into();
        unsafe { callback.Activate(w!("dev.still.notes"), w!("snooze:reminder-id"), &[]) }
            .expect("activate");
        assert!(
            matches!(receiver.try_recv(),Ok(PlatformEvent::Notification(value)) if value=="snooze:reminder-id")
        );
        unsafe { callback.Activate(w!("other.application"), w!("open:note-id"), &[]) }
            .expect("wrong identity ignored");
        assert!(receiver.try_recv().is_err());
    }
}
