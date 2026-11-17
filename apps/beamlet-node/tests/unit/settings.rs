use super::*;
use std::fs;

/// 验证首次配置生成不同固定 UUID，后续加载不生成新身份或写回。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn first_start_creates_distinct_persistent_ids_and_reload_does_not_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let first = Settings::from_file(&path).unwrap();
    assert_eq!(first.color, Color::Auto);
    assert_eq!(first.roles.len(), 2);
    let ids: BTreeSet<_> = std::iter::once(first.node_id)
        .chain(first.roles.iter().map(|role| role.role_id))
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(!ids.contains(&Uuid::nil()));
    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("session_id"));
    let again = Settings::from_file(&path).unwrap();
    assert_eq!(again.node_id, first.node_id);
    assert_eq!(
        again
            .roles
            .iter()
            .map(|role| role.role_id)
            .collect::<Vec<_>>(),
        first
            .roles
            .iter()
            .map(|role| role.role_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

/// 验证同类型角色和基础设施可多次配置且各自参数独立。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn independently_configures_repeated_role_and_infrastructure_types() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    settings.infrastructures.insert(
        "secondary".into(),
        Infrastructure::Nats {
            url: "nats://localhost:4333".into(),
            connect_timeout_secs: 20,
            subscription_capacity: 32,
        },
    );
    let mut extra = settings.roles[1].clone();
    extra.role_id = Uuid::new_v4();
    extra.infrastructure = "secondary".into();
    extra.kind = Role::Worker(Worker {
        pool: "other".parse().unwrap(),
        capabilities: BTreeSet::from([crate::capabilities::UPPERCASE.into()]),
        heartbeat_interval_secs: 1,
        max_concurrent_tasks: 4,
    });
    settings.roles.push(extra);
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    assert_eq!(loaded.roles.len(), 3);
    assert_eq!(loaded.infrastructures.len(), 2);
    let Role::Worker(worker) = &loaded.roles[2].kind else {
        panic!("expected worker")
    };
    assert_eq!(worker.heartbeat_interval_secs, 1);
    assert_eq!(worker.max_concurrent_tasks, 4);
    assert_eq!(worker.pool.to_string(), "other");
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证已有配置缺失或无效角色 UUID 时拒绝启动，不自动补写。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn invalid_or_missing_role_ids_are_not_generated_or_written_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    settings.roles[1].role_id = settings.roles[0].role_id;
    confy::store_path(&path, &settings).unwrap();
    let duplicate = fs::read_to_string(&path).unwrap();
    let distinct = duplicate.replacen(
        &settings.roles[0].role_id.to_string(),
        &Uuid::new_v4().to_string(),
        1,
    );
    for source in [
        duplicate,
        distinct.replace(&format!("role_id: \"{}\",", settings.roles[1].role_id), ""),
        distinct.replace(
            &settings.roles[1].role_id.to_string(),
            &Uuid::nil().to_string(),
        ),
        distinct.replace("infrastructure: \"local\"", "infrastructure: \"missing\""),
        distinct.replace("heartbeat_interval_secs: 5", "heartbeat_interval_secs: 0"),
        distinct.replace("worker_timeout_secs: 15", "worker_timeout_secs: 0"),
        distinct.replace("text.reverse/v1", "text.reverse/v2"),
        distinct.replace("max_concurrent_tasks: 16,", ""),
        distinct.replace("max_concurrent_tasks: 16", "max_concurrent_tasks: -1"),
        distinct.replace("max_concurrent_tasks: 1,", ""),
        distinct.replace("max_concurrent_tasks: 1", "max_concurrent_tasks: -1"),
        distinct.replace("color: auto", "color: other"),
        distinct.replace("role_id:", "session_id:"),
        "invalid RON".into(),
    ] {
        assert_ne!(source, distinct, "invalid fixture must alter the source");
        fs::write(&path, &source).unwrap();
        let error = Settings::from_file(&path).unwrap_err();
        assert!(format!("{error:#}").contains(&path.display().to_string()));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
    }
}

/// 验证 Authority 并发配置按角色独立生效且不写回文件。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn authority_concurrency_is_independent_per_role_and_does_not_rewrite_config() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    let Role::Authority(default) = &settings.roles[0].kind else {
        panic!("expected authority")
    };
    assert_eq!(default.max_concurrent_tasks, 16);
    for limit in [0, 1, 32] {
        let mut role = settings.roles[0].clone();
        role.role_id = Uuid::new_v4();
        let Role::Authority(authority) = &mut role.kind else {
            panic!("expected authority")
        };
        authority.max_concurrent_tasks = limit;
        settings.roles.push(role);
    }
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    let limits: Vec<_> = loaded
        .roles
        .iter()
        .filter_map(|role| match &role.kind {
            Role::Authority(authority) => Some(authority.max_concurrent_tasks),
            Role::Worker(worker) => {
                assert_eq!(worker.max_concurrent_tasks, 1);
                None
            }
        })
        .collect();
    assert_eq!(limits, [16, 0, 1, 32]);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证 Worker 有限/无限并发均可按角色加载且不写回文件。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn worker_concurrency_is_loaded_per_role_including_unlimited_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    let Role::Worker(default) = &settings.roles[1].kind else {
        panic!("expected default worker")
    };
    assert_eq!(default.max_concurrent_tasks, 1);
    for limit in [0, 4, 32] {
        let mut role = settings.roles[1].clone();
        role.role_id = Uuid::new_v4();
        let Role::Worker(worker) = &mut role.kind else {
            panic!("expected worker")
        };
        worker.max_concurrent_tasks = limit;
        settings.roles.push(role);
    }
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    let limits: Vec<_> = loaded
        .roles
        .iter()
        .filter_map(|role| match &role.kind {
            Role::Worker(worker) => Some(worker.max_concurrent_tasks),
            Role::Authority(_) => None,
        })
        .collect();
    assert_eq!(limits, [1, 0, 4, 32]);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证 pool 去重只作用于内存，保留原配置文本。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn pool_duplicates_are_deduplicated_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let settings = Settings::default();
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path)
        .unwrap()
        .replace("pools: [", "pools: [\"default\", ");
    fs::write(&path, &source).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    let Role::Authority(authority) = &loaded.roles[0].kind else {
        panic!("expected authority")
    };
    assert_eq!(authority.pools.len(), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证跨角色种类的重复 UUID 和未知基础设施引用被拒绝。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn duplicate_ids_across_role_types_and_unknown_infrastructure_are_rejected() {
    let mut settings = Settings::default();
    settings.roles[1].role_id = settings.roles[0].role_id;
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("重复的 role_id")
    );
    settings.roles[1].role_id = Uuid::new_v4();
    settings.roles[1].infrastructure = "not configured".into();
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("不存在的基础设施")
    );
    settings.roles.clear();
    settings.validate().unwrap();
}

/// 验证同进程并发首次加载复用已写入的全部固定 UUID。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn concurrent_first_starts_reuse_all_persisted_ids() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let barrier = std::sync::Barrier::new(8);
    let ids = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    let settings = Settings::from_file(&path).unwrap();
                    (
                        settings.node_id,
                        settings
                            .roles
                            .iter()
                            .map(|role| role.role_id)
                            .collect::<Vec<_>>(),
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(ids.iter().all(|id| *id == ids[0]));
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
    let path = directory.path().join("missing/node.ron");
    let error = Settings::from_file(&path).unwrap_err();
    assert!(error.to_string().contains(&path.display().to_string()));
}
/// 验证退出/报告/连接期限及传输容量必填、为正且不会自动写回。
///
/// # 参数
///
/// 无。
///
/// # 返回值
///
/// `()`；全部断言通过时正常结束，断言失败或未预期错误会使测试失败。
#[test]
fn deadlines_and_transport_limits_are_required_positive_and_not_written_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings {
        shutdown_timeout_secs: 30,
        ..Settings::default()
    };
    let Role::Authority(authority) = &mut settings.roles[0].kind else {
        panic!("authority")
    };
    authority.report_timeout_secs = 25;
    authority.input_capacity = 7;
    authority.max_concurrent_replies = 3;
    let Infrastructure::Nats {
        connect_timeout_secs,
        subscription_capacity,
        ..
    } = settings.infrastructures.get_mut("local").unwrap();
    *connect_timeout_secs = 20;
    *subscription_capacity = 128;
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    assert_eq!(loaded.shutdown_timeout_secs, 30);
    let Role::Authority(authority) = &loaded.roles[0].kind else {
        panic!("authority")
    };
    assert_eq!(authority.report_timeout_secs, 25);
    assert_eq!(
        (authority.input_capacity, authority.max_concurrent_replies),
        (7, 3)
    );
    let Infrastructure::Nats {
        connect_timeout_secs,
        subscription_capacity,
        ..
    } = &loaded.infrastructures["local"];
    assert_eq!((*connect_timeout_secs, *subscription_capacity), (20, 128));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    for field in [
        "shutdown_timeout_secs: 30",
        "report_timeout_secs: 25",
        "connect_timeout_secs: 20",
        "subscription_capacity: 128",
        "input_capacity: 7",
        "max_concurrent_replies: 3",
    ] {
        let name = field.split(':').next().unwrap();
        for replacement in [
            String::new(),
            format!("{name}: 0"),
            format!("{name}: -1"),
            format!("{name}: {}", u64::MAX),
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
