//! Start at logon with administrator rights through a Task Scheduler task.
//!
//! A `Run` registry entry cannot start an elevated process without a UAC prompt, so the
//! application registers a task with `RunLevel=HighestAvailable` and a logon trigger for the
//! current user. Registering such a task requires administrator rights.

pub fn task_name() -> &'static str {
    if cfg!(debug_assertions) {
        "Remote HTTP Action (dev)"
    } else {
        "Remote HTTP Action"
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Task definition: logon trigger for `user_sid`, highest available rights, no time limit,
/// runs on battery, one instance.
pub fn task_xml(exe: &str, working_dir: &str, user_sid: &str) -> String {
    let (exe, dir, sid) = (
        xml_escape(exe),
        xml_escape(working_dir),
        xml_escape(user_sid),
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Starts Remote HTTP Action at logon with administrator rights.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{sid}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{sid}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{exe}</Command>
      <Arguments>--minimized</Arguments>
      <WorkingDirectory>{dir}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

/// Extracts the `<Command>` value from an exported task definition.
pub fn command_from_xml(xml: &str) -> Option<String> {
    let start = xml.find("<Command>")? + "<Command>".len();
    let end = xml[start..].find("</Command>")? + start;
    Some(
        xml[start..end]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&"),
    )
}

pub fn same_path(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_matches('"').replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

#[cfg(windows)]
mod imp {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    use super::*;
    use crate::error::{AppError, AppResult, ErrorCode};
    use crate::platform::AutostartStatus;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn schtasks(args: &[&str]) -> AppResult<(bool, String)> {
        let out = Command::new("schtasks.exe")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| AppError::internal(format!("cannot run schtasks.exe: {e}")))?;
        let mut text = crate::platform::system::decode(&out.stdout);
        text.push_str(&crate::platform::system::decode(&out.stderr));
        Ok((out.status.success(), text.trim().to_string()))
    }

    fn current_exe() -> AppResult<String> {
        Ok(std::env::current_exe()
            .map_err(|e| AppError::internal(e.to_string()))?
            .display()
            .to_string())
    }

    pub fn status(name: &str) -> AppResult<AutostartStatus> {
        let (ok, text) = schtasks(&["/Query", "/TN", name, "/XML", "ONE"])?;
        if !ok {
            return Ok(AutostartStatus {
                supported: true,
                installed: false,
                command: None,
                matches_current: false,
            });
        }
        let command = command_from_xml(&text);
        let exe = current_exe()?;
        Ok(AutostartStatus {
            supported: true,
            installed: true,
            matches_current: command.as_deref().is_some_and(|c| same_path(c, &exe)),
            command,
        })
    }

    pub fn install(name: &str) -> AppResult<()> {
        if !crate::platform::win::is_elevated() {
            return Err(AppError::new(
                ErrorCode::AdminRequired,
                "setting up autostart requires administrator rights; restart the application as administrator",
            ));
        }
        let exe = current_exe()?;
        let dir = std::path::Path::new(&exe)
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let sid = crate::platform::win::current_user_sid()
            .ok_or_else(|| AppError::internal("cannot determine the current user"))?;
        let xml = task_xml(&exe, &dir, &sid);

        // schtasks expects the definition as UTF-16 LE with a BOM.
        let file = std::env::temp_dir().join(format!("rha-task-{}.xml", uuid::Uuid::new_v4()));
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        std::fs::write(&file, bytes)?;
        let result = schtasks(&[
            "/Create",
            "/TN",
            name,
            "/XML",
            &file.display().to_string(),
            "/F",
        ]);
        let _ = std::fs::remove_file(&file);
        let (ok, text) = result?;
        if !ok {
            return Err(AppError::internal(format!("schtasks failed: {text}")));
        }
        Ok(())
    }

    pub fn remove(name: &str) -> AppResult<()> {
        if !status(name)?.installed {
            return Ok(());
        }
        let (ok, text) = schtasks(&["/Delete", "/TN", name, "/F"])?;
        if !ok {
            let code = if crate::platform::win::is_elevated() {
                ErrorCode::Internal
            } else {
                ErrorCode::AdminRequired
            };
            return Err(AppError::new(code, format!("schtasks failed: {text}")));
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Registers and removes a real task. Needs an elevated session:
        /// `cargo test -- --ignored autostart_round_trip`.
        #[test]
        #[ignore]
        fn autostart_round_trip() {
            let name = format!("rha-test-{}", uuid::Uuid::new_v4());
            assert!(!status(&name).unwrap().installed);
            install(&name).unwrap();
            let s = status(&name).unwrap();
            assert!(s.installed, "{s:?}");
            assert!(s.matches_current, "{s:?}");
            remove(&name).unwrap();
            assert!(!status(&name).unwrap().installed);
        }
    }
}

#[cfg(windows)]
pub use imp::{install, remove, status};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_xml_contains_elevated_logon_trigger() {
        let xml = task_xml(r"C:\Apps\R&D\app.exe", r"C:\Apps\R&D", "S-1-5-21-1");
        assert!(xml.contains("<RunLevel>HighestAvailable</RunLevel>"));
        assert!(xml.contains("<LogonTrigger>"));
        assert!(xml.contains("<UserId>S-1-5-21-1</UserId>"));
        assert!(xml.contains("<Arguments>--minimized</Arguments>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains(r"<Command>C:\Apps\R&amp;D\app.exe</Command>"));
        assert_eq!(command_from_xml(&xml).unwrap(), r"C:\Apps\R&D\app.exe");
    }

    #[test]
    fn paths_are_compared_case_insensitively() {
        assert!(same_path(r#""C:\Apps\App.exe""#, r"c:\apps\app.EXE"));
        assert!(!same_path(r"C:\a.exe", r"C:\b.exe"));
    }
}
