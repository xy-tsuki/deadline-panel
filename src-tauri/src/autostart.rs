//! Read and write startup settings without launching a console process.
use windows::{
    core::HSTRING,
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_MORE_DATA},
        System::Registry::{
            RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
            RRF_RT_REG_SZ,
        },
    },
};

pub fn read(key: &str, name: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let key = HSTRING::from(key);
    let name = HSTRING::from(name);
    let mut buffer = vec![0u16; 1024];
    for _ in 0..3 {
        let mut bytes = (buffer.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key,
                &name,
                RRF_RT_REG_SZ,
                None,
                Some(buffer.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if status == ERROR_MORE_DATA && bytes <= 65_536 {
            buffer.resize((bytes as usize + 1) / 2, 0);
            continue;
        }
        status.ok()?;
        let length = buffer
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(buffer.len());
        return Ok(Some(String::from_utf16(&buffer[..length])?));
    }
    Err("Startup registry value changed repeatedly while reading".into())
}

pub fn write(key: &str, name: &str, value: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let key = HSTRING::from(key);
    let name = HSTRING::from(name);
    let status = if let Some(value) = value {
        let data: Vec<u16> = value.encode_utf16().chain(Some(0)).collect();
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                &name,
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        }
    } else {
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &key, &name) }
    };
    if value.is_none() && status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok()?;
    Ok(())
}

pub fn matches_executable(command: &str, executable: &str) -> bool {
    let command = command.trim().to_lowercase();
    let executable = executable.to_lowercase();
    let expected = if command.starts_with('"') {
        format!("\"{executable}\"")
    } else {
        executable
    };
    command
        .strip_prefix(&expected)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_roundtrip_preserves_unicode_and_other_values() {
        // Never touch the real Run key or the user's startup entry.
        let key = format!(
            r"Software\DeadlinePanelTests\Autostart-{}",
            std::process::id()
        );
        assert_eq!(read(&key, "Deadline Panel").unwrap(), None);
        write(&key, "Other app", Some("unchanged")).unwrap();
        let command = r#""E:\应用 程序\deadline-panel.exe""#;
        write(&key, "Deadline Panel", Some(command)).unwrap();
        assert_eq!(
            read(&key, "Deadline Panel").unwrap().as_deref(),
            Some(command)
        );
        let long = "中文".repeat(1500);
        write(&key, "Deadline Panel", Some(&long)).unwrap();
        assert_eq!(read(&key, "Deadline Panel").unwrap(), Some(long));
        write(&key, "Deadline Panel", None).unwrap();
        write(&key, "Deadline Panel", None).unwrap();
        assert_eq!(read(&key, "Deadline Panel").unwrap(), None);
        assert_eq!(
            read(&key, "Other app").unwrap().as_deref(),
            Some("unchanged")
        );
        write(&key, "Other app", None).unwrap();
        unsafe {
            windows::Win32::System::Registry::RegDeleteKeyW(HKEY_CURRENT_USER, &HSTRING::from(key))
                .ok()
                .unwrap();
        }
    }

    #[test]
    fn startup_path_matching_does_not_accept_another_executable() {
        let path = r"E:\Apps\Deadline Panel\deadline-panel.exe";
        assert!(matches_executable(&format!("\"{path}\""), path));
        assert!(matches_executable(
            &format!("\"{}\" --quiet", path.to_uppercase()),
            path
        ));
        assert!(matches_executable(path, path));
        assert!(!matches_executable(&format!("{path}.old"), path));
        assert!(!matches_executable(&format!("\"{path}\".old"), path));
    }
}
