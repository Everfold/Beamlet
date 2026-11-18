use super::*;
use std::fs;

/// 验证首次配置生成不同固定 UUID, 后续加载不生成新身份或写回.
#[test]
fn first_start_creates_distinct_persistent_ids_and_reload_does_not_write() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let first = Settings::from_file(&path).unwrap();
    assert_eq!(first.node.color, Color::Auto);
    assert_eq!(first.roles.len(), 2);
    let ids: BTreeSet<_> = std::iter::once(first.node.node_id)
        .chain(first.roles.iter().map(Role::id))
        .collect();
    assert_eq!(ids.len(), 3);
    assert!(!ids.contains(&Uuid::nil()));
    let content = fs::read_to_string(&path).unwrap();
    assert!(!content.contains("session_id"));
    let lines: Vec<_> = content.lines().map(str::trim).collect();
    for (field, note, count) in [
        ("role_id:", "固定 UUID / Unique persistent UUID", 2),
        ("subscription_capacity:", "消息数 / Buffered messages", 1),
        ("max_concurrent_tasks:", "0 = unlimited", 2),
    ] {
        assert_eq!(
            lines
                .windows(2)
                .filter(|pair| pair[0].starts_with("/// ")
                    && pair[0].contains(note)
                    && pair[1].starts_with(field))
                .count(),
            count,
            "{field}"
        );
    }
    let content = format!("// 自定义备注 / My note\n{content}");
    fs::write(&path, &content).unwrap();
    let again = Settings::from_file(&path).unwrap();
    assert_eq!(again.node.node_id, first.node.node_id);
    assert_eq!(
        again.roles.iter().map(Role::id).collect::<Vec<_>>(),
        first.roles.iter().map(Role::id).collect::<Vec<_>>()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), content);
}

/// 验证同类型角色和基础设施可多次配置且各自参数独立.
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
    settings.roles.push(Role::Worker {
        role_id: Uuid::new_v4(),
        label: Some("secondary worker".into()),
        infrastructures: ["secondary".into()].into(),
        pool: "other".parse().unwrap(),
        capabilities: BTreeSet::from([crate::capabilities::UPPERCASE.into()]),
        heartbeat_interval_secs: 1,
        max_concurrent_tasks: 4,
    });
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    assert_eq!(loaded.roles.len(), 3);
    assert_eq!(loaded.infrastructures.len(), 2);
    let Role::Worker {
        heartbeat_interval_secs,
        max_concurrent_tasks,
        pool,
        ..
    } = &loaded.roles[2]
    else {
        panic!("expected worker")
    };
    assert_eq!(*heartbeat_interval_secs, 1);
    assert_eq!(*max_concurrent_tasks, 4);
    assert_eq!(pool.to_string(), "other");
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证已有配置缺失或无效角色 UUID 时拒绝启动, 不自动补写.
#[test]
fn invalid_or_missing_role_ids_are_not_generated_or_written_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    let duplicate_id = settings.roles[0].id();
    let Role::Worker { role_id, .. } = &mut settings.roles[1] else {
        panic!("expected worker")
    };
    *role_id = duplicate_id;
    confy::store_path(&path, &settings).unwrap();
    let duplicate = fs::read_to_string(&path).unwrap();
    let distinct = duplicate.replacen(
        &settings.roles[0].id().to_string(),
        &Uuid::new_v4().to_string(),
        1,
    );
    for source in [
        duplicate,
        distinct.replace(&format!("role_id: \"{}\",", settings.roles[1].id()), ""),
        distinct.replace(
            &settings.roles[1].id().to_string(),
            &Uuid::nil().to_string(),
        ),
        distinct.replace("\"nats\",", "\"missing\","),
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

/// 验证 Authority 并发配置按角色独立生效且不写回文件.
#[test]
fn authority_concurrency_is_independent_per_role_and_does_not_rewrite_config() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    let Role::Authority {
        limits: default, ..
    } = &settings.roles[0]
    else {
        panic!("expected authority")
    };
    assert_eq!(default.max_concurrent_tasks, 16);
    for limit in [0, 1, 32] {
        let mut role = settings.roles[0].clone();
        let Role::Authority {
            limits, role_id, ..
        } = &mut role
        else {
            panic!("expected authority")
        };
        *role_id = Uuid::new_v4();
        limits.max_concurrent_tasks = limit;
        settings.roles.push(role);
    }
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    let limits: Vec<_> = loaded
        .roles
        .iter()
        .filter_map(|role| match role {
            Role::Authority { limits, .. } => Some(limits.max_concurrent_tasks),
            Role::Worker {
                max_concurrent_tasks,
                ..
            } => {
                assert_eq!(*max_concurrent_tasks, 1);
                None
            }
        })
        .collect();
    assert_eq!(limits, [16, 0, 1, 32]);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证 Worker 有限/无限并发均可按角色加载且不写回文件.
#[test]
fn worker_concurrency_is_loaded_per_role_including_unlimited_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    let Role::Worker {
        max_concurrent_tasks,
        ..
    } = &settings.roles[1]
    else {
        panic!("expected default worker")
    };
    assert_eq!(*max_concurrent_tasks, 1);
    for limit in [0, 4, 32] {
        let mut role = settings.roles[1].clone();
        let Role::Worker {
            max_concurrent_tasks,
            role_id,
            ..
        } = &mut role
        else {
            panic!("expected worker")
        };
        *role_id = Uuid::new_v4();
        *max_concurrent_tasks = limit;
        settings.roles.push(role);
    }
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    let limits: Vec<_> = loaded
        .roles
        .iter()
        .filter_map(|role| match role {
            Role::Worker {
                max_concurrent_tasks,
                ..
            } => Some(*max_concurrent_tasks),
            Role::Authority { .. } => None,
        })
        .collect();
    assert_eq!(limits, [1, 0, 4, 32]);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证 pool 去重只作用于内存, 保留原配置文本.
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
    let Role::Authority { pools, .. } = &loaded.roles[0] else {
        panic!("expected authority")
    };
    assert_eq!(pools.len(), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
}

/// 验证跨角色种类的重复 UUID 和未知基础设施引用被拒绝.
#[test]
fn duplicate_ids_across_role_types_and_unknown_infrastructure_are_rejected() {
    let mut settings = Settings::default();
    let duplicate_id = settings.roles[0].id();
    let Role::Worker { role_id, .. } = &mut settings.roles[1] else {
        panic!("expected worker")
    };
    *role_id = duplicate_id;
    assert!(
        settings
            .validate()
            .unwrap_err()
            .to_string()
            .contains("重复的 role_id")
    );
    let Role::Worker {
        role_id,
        infrastructures,
        ..
    } = &mut settings.roles[1]
    else {
        panic!("expected worker")
    };
    *role_id = Uuid::new_v4();
    *infrastructures = ["not configured".into()].into();
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

/// 验证同进程并发首次加载复用已写入的全部固定 UUID.
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
                        settings.node.node_id,
                        settings.roles.iter().map(Role::id).collect::<Vec<_>>(),
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

/// 验证首次创建配置失败时错误包含目标路径.
#[test]
fn reports_config_creation_failure_with_path() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("missing/node.ron");
    let error = Settings::from_file(&path).unwrap_err();
    assert!(error.to_string().contains(&path.display().to_string()));
}
/// 验证退出/报告/连接期限及传输容量必填, 为正且不会自动写回.
#[test]
fn deadlines_and_transport_limits_are_required_positive_and_not_written_back() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    settings.node.shutdown_timeout_secs = 30;
    let Role::Authority {
        limits, timeouts, ..
    } = &mut settings.roles[0]
    else {
        panic!("authority")
    };
    timeouts.report_timeout_secs = 25;
    limits.input_capacity = 7;
    limits.max_concurrent_replies = 3;
    let Infrastructure::Nats {
        connect_timeout_secs,
        subscription_capacity,
        ..
    } = settings.infrastructures.get_mut("nats").unwrap()
    else {
        panic!("expected NATS")
    };
    *connect_timeout_secs = 20;
    *subscription_capacity = 128;
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    assert_eq!(loaded.node.shutdown_timeout_secs, 30);
    let Role::Authority {
        limits, timeouts, ..
    } = &loaded.roles[0]
    else {
        panic!("authority")
    };
    assert_eq!(timeouts.report_timeout_secs, 25);
    assert_eq!(
        (limits.input_capacity, limits.max_concurrent_replies),
        (7, 3)
    );
    let Infrastructure::Nats {
        connect_timeout_secs,
        subscription_capacity,
        ..
    } = &loaded.infrastructures["nats"]
    else {
        panic!("expected NATS")
    };
    assert_eq!((*connect_timeout_secs, *subscription_capacity), (20, 128));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    for field in [
        "shutdown_timeout_secs: 30",
        "worker_timeout_secs: 15",
        "heartbeat_interval_secs: 5",
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

/// Local 容量和角色引用在启动前校验, 重复引用去重且不写回配置.
#[test]
fn local_and_multiple_transports_are_validated_without_rewriting() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("node.ron");
    let mut settings = Settings::default();
    settings
        .infrastructures
        .insert("local".into(), Infrastructure::Local { capacity: 7 });
    let Role::Authority {
        infrastructures, ..
    } = &mut settings.roles[0]
    else {
        panic!("expected authority")
    };
    infrastructures.insert("local".into());
    let Role::Worker {
        infrastructures, ..
    } = &mut settings.roles[1]
    else {
        panic!("expected worker")
    };
    *infrastructures = ["local".into()].into();
    confy::store_path(&path, &settings).unwrap();
    let source = fs::read_to_string(&path)
        .unwrap()
        .replace("infrastructures: [", "infrastructures: [\"local\", ");
    fs::write(&path, &source).unwrap();
    let loaded = Settings::from_file(&path).unwrap();
    assert_eq!(loaded.roles[0].infrastructures().len(), 2);
    assert_eq!(loaded.roles[1].infrastructures().len(), 1);
    assert!(matches!(
        loaded.infrastructures["local"],
        Infrastructure::Local { capacity: 7 }
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    for replacement in [
        "capacity: 0".into(),
        format!("capacity: {}", usize::MAX),
        "".into(),
    ] {
        let invalid = source.replace("capacity: 7", &replacement);
        fs::write(&path, &invalid).unwrap();
        assert!(Settings::from_file(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
    }
    fs::write(&path, &source).unwrap();
    for (index, names, reason) in [
        (1, vec!["local", "nats"], "只能引用一个基础设施"),
        (1, vec![], "至少需要一个基础设施"),
        (0, vec![], "至少需要一个基础设施"),
    ] {
        let mut settings = Settings::from_file(&path).unwrap();
        let infrastructures = match &mut settings.roles[index] {
            Role::Authority {
                infrastructures, ..
            }
            | Role::Worker {
                infrastructures, ..
            } => infrastructures,
        };
        *infrastructures = names.into_iter().map(String::from).collect();
        assert!(
            settings
                .validate()
                .unwrap_err()
                .to_string()
                .contains(reason)
        );
    }
}
