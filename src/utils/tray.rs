//! Detection of apps that show an icon in the system tray (StatusNotifierItem).

use std::fs;

const WATCHERS: [&str; 2] = [
    "org.kde.StatusNotifierWatcher",
    "org.freedesktop.StatusNotifierWatcher",
];

/// Returns whether the process `pid`, or one of its descendants, has a registered tray item.
pub fn has_tray_item(pid: i32) -> anyhow::Result<bool> {
    use std::sync::OnceLock;
    use std::time::Duration;

    use anyhow::Context as _;
    use zbus::names::BusName;
    use zbus::zvariant::OwnedValue;

    static CONNECTION: OnceLock<zbus::Result<zbus::blocking::Connection>> = OnceLock::new();
    let conn = CONNECTION
        .get_or_init(|| {
            zbus::blocking::connection::Builder::session()?
                .method_timeout(Duration::from_secs(1))
                .build()
        })
        .clone()
        .context("error connecting to session bus")?;

    let dbus = zbus::blocking::fdo::DBusProxy::new(&conn).context("error creating a Proxy")?;

    for watcher in WATCHERS {
        let reply = conn.call_method(
            Some(watcher),
            "/StatusNotifierWatcher",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(watcher, "RegisteredStatusNotifierItems"),
        );
        // The watcher isn't running under this name, try the next one.
        let Ok(reply) = reply else {
            continue;
        };

        let items: OwnedValue = reply
            .body()
            .deserialize()
            .context("error deserializing RegisteredStatusNotifierItems")?;
        let items = Vec::<String>::try_from(items)
            .context("error deserializing RegisteredStatusNotifierItems")?;

        for item in items {
            let Some(service) = item_service(&item) else {
                continue;
            };
            let Ok(name) = BusName::try_from(service) else {
                continue;
            };
            let Ok(item_pid) = dbus.get_connection_unix_process_id(name) else {
                continue;
            };
            let Ok(item_pid) = i32::try_from(item_pid) else {
                continue;
            };
            if is_same_or_descendant(item_pid, pid) {
                return Ok(true);
            }
        }

        return Ok(false);
    }

    Ok(false)
}

/// Extracts the D-Bus service name from a registered tray item.
///
/// Watchers list items either as a bare service name or as a service name followed by the object
/// path, e.g. `:1.42/org/ayatana/NotificationItem/app`.
fn item_service(item: &str) -> Option<&str> {
    let service = match item.find('/') {
        Some(idx) => &item[..idx],
        None => item,
    };
    (!service.is_empty()).then_some(service)
}

/// Returns whether `pid` is `ancestor` or one of its descendant processes.
fn is_same_or_descendant(mut pid: i32, ancestor: i32) -> bool {
    if ancestor <= 1 {
        return pid == ancestor;
    }

    // Bound the walk in case of weird /proc contents.
    for _ in 0..64 {
        if pid == ancestor {
            return true;
        }
        if pid <= 1 {
            return false;
        }
        let Some(parent) = fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|stat| parse_parent_pid(&stat))
        else {
            return false;
        };
        pid = parent;
    }

    false
}

/// Parses the parent PID out of the contents of `/proc/<pid>/stat`.
fn parse_parent_pid(stat: &str) -> Option<i32> {
    // The process name is in parentheses and can contain spaces and parentheses itself, so look
    // for the fields after the last closing parenthesis: state, then the parent PID.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_service_parsing() {
        assert_eq!(
            item_service("org.kde.StatusNotifierItem-1234-1/StatusNotifierItem"),
            Some("org.kde.StatusNotifierItem-1234-1")
        );
        assert_eq!(
            item_service(":1.42/org/ayatana/NotificationItem/app"),
            Some(":1.42")
        );
        assert_eq!(item_service(":1.42"), Some(":1.42"));
        assert_eq!(item_service("/StatusNotifierItem"), None);
        assert_eq!(item_service(""), None);
    }

    #[test]
    fn parent_pid_parsing() {
        assert_eq!(
            parse_parent_pid("1234 (telegram-desktop) S 1000 1234 1234 0 -1"),
            Some(1000)
        );
        assert_eq!(
            parse_parent_pid("1234 (weird) name (x)) R 42 1234 1234 0 -1"),
            Some(42)
        );
        assert_eq!(parse_parent_pid("garbage"), None);
    }

    #[test]
    fn current_process_is_a_descendant_of_its_parent() {
        let pid = std::process::id() as i32;
        assert!(is_same_or_descendant(pid, pid));

        let stat = fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let parent = parse_parent_pid(&stat).unwrap();
        if parent > 1 {
            assert!(is_same_or_descendant(pid, parent));
            assert!(!is_same_or_descendant(parent, pid));
        }
    }
}
