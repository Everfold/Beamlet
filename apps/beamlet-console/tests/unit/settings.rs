use super::*;
use std::fs;

/// 验证已有 Console 配置被正确加载且文件内容不被改写。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn loads_existing_config_without_overwriting_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("console.ron");
    for color in [Color::Always, Color::Never, Color::Auto] {
        let config = Settings {
            nats_url: "nats://localhost:4333".into(),
            color,
            request_timeout_secs: 30,
            discovery_window_ms: 250,
            max_in_flight_requests: 7,
            connect_timeout_secs: 20,
            subscription_capacity: 32,
            ..Settings::default()
        };
        confy::store_path(&path, &config).unwrap();
        let source = fs::read_to_string(&path).unwrap();
        let settings = Settings::from_file(&path).unwrap();
        assert_eq!(settings.nats_url, config.nats_url);
        assert_eq!(settings.color, color);
        assert_eq!(settings.request_timeout_secs, 30);
        assert_eq!(settings.discovery_window_ms, 250);
        assert_eq!(settings.max_in_flight_requests, 7);
        assert_eq!(settings.connect_timeout_secs, 20);
        assert_eq!(settings.subscription_capacity, 32);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

/// 验证未知配置字段和非法 RON 会报错并保留原文件。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn rejects_unknown_fields_and_invalid_ron() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("console.ron");
    for source in [
        r#"(nats_ur1: "nats://localhost:4333")"#,
        r#"(nats_url: "nats://localhost:4333""#,
        "(color: invalid)",
    ] {
        fs::write(&path, source).unwrap();
        let error = Settings::from_file(&path).unwrap_err();
        assert!(error.to_string().contains(&path.display().to_string()));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

/// 验证首次启动创建 Console 配置，后续加载复用原文件。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn creates_and_loads_config_on_first_start() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("console.ron");
    let settings = Settings::from_file(&path).unwrap();
    assert_eq!(settings.nats_url, "nats://127.0.0.1:4222");
    assert_eq!(settings.color, Color::Auto);

    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.is_empty(), "首次启动应创建配置文件");
    let reloaded = Settings::from_file(&path).unwrap();
    assert_eq!(reloaded.nats_url, settings.nats_url);
    assert_eq!(reloaded.color, settings.color);
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

/// 验证首次创建配置失败时错误包含目标路径。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn reports_config_creation_failure_with_path() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("not-a-directory");
    fs::write(&parent, "occupied").unwrap();
    let path = parent.join("console.ron");
    let error = Settings::from_file(&path).unwrap_err();
    assert!(error.to_string().contains(&path.display().to_string()));
}
/// 验证期限/容量字段缺失、零值、负值或溢出时拒绝配置且不写回。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn deadlines_and_capacity_reject_missing_zero_negative_and_overflow_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("console.ron");
    confy::store_path(&path, Settings::default()).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    for field in [
        "request_timeout_secs: 10",
        "connect_timeout_secs: 10",
        "subscription_capacity: 65536",
        "discovery_window_ms: 1000",
        "max_in_flight_requests: 16",
    ] {
        let name = field.split(':').next().unwrap();
        for replacement in [
            String::new(),
            format!("{name}: 0"),
            format!("{name}: -1"),
            format!("{name}: 18446744073709551616"),
        ] {
            let invalid = source.replace(
                &format!("{field},"),
                &if replacement.is_empty() {
                    replacement
                } else {
                    format!("{replacement},")
                },
            );
            assert_ne!(invalid, source);
            fs::write(&path, &invalid).unwrap();
            assert!(
                Settings::from_file(&path).is_err(),
                "accepted {name}: {invalid}"
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
    }
}
/// 验证发现窗口严格小于总请求期限，为通信准备留出时间。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn discovery_window_must_leave_time_for_network_setup() {
    let mut settings = Settings {
        request_timeout_secs: 2,
        discovery_window_ms: 1500,
        max_in_flight_requests: 3,
        ..Default::default()
    };
    settings.validate().unwrap();
    for window in [2000, 2001, u64::MAX] {
        settings.discovery_window_ms = window;
        assert!(settings.validate().is_err());
    }
}
