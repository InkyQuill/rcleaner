//! PowerShell's ScheduledTasks module registers a task for the current user.
use super::checked;
use anyhow::Result;
use std::process::Command;

fn powershell(script: &str) -> Result<String> {
    let output = checked(Command::new("powershell.exe").args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        script,
    ]))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}
// The SID isolates task names for multiple users on the same machine.
const PRELUDE: &str = "$ErrorActionPreference='Stop'; $id=[System.Security.Principal.WindowsIdentity]::GetCurrent(); $name='Rcleaner-'+$id.User.Value; ";

fn ps_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

// Windows CommandLineToArgvW/CRT quoting, including trailing backslashes.
fn quote_arg(value: &str) -> String {
    let mut out = String::from("\"");
    let mut slashes = 0;
    for ch in value.chars() {
        if ch == '\\' {
            slashes += 1;
            continue;
        }
        if ch == '"' {
            out.push_str(&"\\".repeat(slashes * 2 + 1));
        } else {
            out.push_str(&"\\".repeat(slashes));
        }
        slashes = 0;
        out.push(ch);
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}

fn registration(weekday: u32, hour: u32, days: u32, root: &str, binary: &str) -> String {
    let day = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ][weekday as usize];
    let arguments = ps_literal(&format!("sweep --days {days} --root {}", quote_arg(root)));
    let binary = ps_literal(binary);
    format!("{PRELUDE}\
$action=New-ScheduledTaskAction -Execute {binary} -Argument {arguments}; \
$trigger=New-ScheduledTaskTrigger -Weekly -DaysOfWeek {day} -At ([DateTime]::Today.AddHours({hour})); \
$principal=New-ScheduledTaskPrincipal -UserId $id.User.Value -LogonType Interactive -RunLevel Limited; \
$settings=New-ScheduledTaskSettingsSet -StartWhenAvailable -MultipleInstances IgnoreNew; \
Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Force | Out-Null;")
}

pub fn enable(weekday: u32, hour: u32, days: u32, root: &str, binary: &str) -> Result<()> {
    powershell(&registration(weekday, hour, days, root, binary))?;
    Ok(())
}
pub fn disable() -> Result<()> {
    powershell(&format!("{PRELUDE}$task=Get-ScheduledTask | Where-Object {{$_.TaskName -eq $name -and $_.TaskPath -eq '\\'}}; if ($task) {{ $task | Unregister-ScheduledTask -Confirm:$false }}"))?;
    Ok(())
}
pub fn is_loaded() -> Result<bool> {
    let result = powershell(&format!("{PRELUDE}$task=Get-ScheduledTask | Where-Object {{$_.TaskName -eq $name -and $_.TaskPath -eq '\\'}}; if ($task -and $task.State -ne 'Disabled') {{ 'enabled' }} else {{ 'disabled' }}"))?;
    Ok(result == "enabled")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_arguments_preserve_spaces_quotes_and_trailing_slashes() {
        assert_eq!(quote_arg("C:\\a b\\"), "\"C:\\a b\\\\\"");
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
        assert_eq!(ps_literal("a'b;$x"), "'a''b;$x'");
    }
    #[test]
    fn registration_is_per_user_and_escapes_paths() {
        let script = registration(
            6,
            23,
            45,
            "C:\\User's projects\\",
            "C:\\Русский путь\\oxy.exe",
        );
        assert!(script.contains("-DaysOfWeek Saturday"));
        assert!(script.contains("-LogonType Interactive -RunLevel Limited"));
        assert!(script.contains("User''s projects"));
        assert!(script.contains("-Execute 'C:\\Русский путь\\oxy.exe'"));
    }
}
