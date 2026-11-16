//! 真实启动入口验证 Node 与各角色的持久身份和独立运行会话.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};
use uuid::Uuid;

fn field(line: &str, name: &str) -> Uuid {
    line.split_whitespace()
        .find_map(|field| field.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("missing {name}: {line}"))
        .trim_end_matches('}')
        .parse()
        .unwrap()
}
fn start(binary: &Path, cwd: &Path, args: &[&str]) -> (String, Uuid, BTreeMap<Uuid, Uuid>) {
    let output = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    // 无效 URL 让测试在网络连接前退出, 但所有角色会话已经分配并输出.
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let log = String::from_utf8(output.stderr).unwrap();
    assert!(log.contains("连接 NATS 失败"), "{log}");
    let startup = log
        .lines()
        .find(|line| line.contains("event=\"node.starting\""))
        .expect("node startup log");
    let node_session = field(startup, "node_session_id");
    let roles = log
        .lines()
        .filter(|line| line.contains("event=\"role.prepared\""))
        .map(|line| (field(line, "role_id"), field(line, "role_session_id")))
        .collect();
    (log, node_session, roles)
}

#[test]
fn restarts_preserve_each_role_id_and_renew_separate_sessions_without_writing_config() {
    for (file_name, args) in [
        ("node.ron", &[][..]),
        ("multi.ron", &["--config", "multi.ron"][..]),
        ("组合节点.ron", &["-c", "组合节点.ron"][..]),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        fs::write(cwd.path().join(file_name), "invalid cwd config").unwrap();
        let binary = directory
            .path()
            .join(format!("beamlet-node{}", std::env::consts::EXE_SUFFIX));
        fs::copy(env!("CARGO_BIN_EXE_beamlet-node"), &binary).unwrap();
        let path = directory.path().join(file_name);
        if file_name != "node.ron" {
            fs::write(directory.path().join("node.ron"), "unselected").unwrap();
        }
        let node_id = Uuid::new_v4();
        let role_ids = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
        let roles: Vec<_> = role_ids.iter().enumerate().map(|(i, id)| {
            let kind = if i == 0 { "authority((pools: [\"default\"], worker_timeout_secs: 15, report_timeout_secs: 3, input_capacity: 16, max_concurrent_replies: 16, max_concurrent_tasks: 16))" } else { "worker((pool: \"default\", capabilities: [\"text.reverse/v1\"], heartbeat_interval_secs: 5, max_concurrent_tasks: 1))" };
            format!(r#"(role_id: "{id}", label: Some("角色{i}"), infrastructure: "local", kind: {kind})"#)
        }).collect();
        let source = format!(
            r#"(node_id: "{node_id}", label: Some("节点甲"), color: never, shutdown_timeout_secs: 8, infrastructures: {{"local": nats(url: "nats://[invalid", connect_timeout_secs: 12, subscription_capacity: 32)}}, roles: [{}])"#,
            roles.join(",")
        );
        fs::write(&path, &source).unwrap();
        let (first_log, first_node_session, first_roles) = start(&binary, cwd.path(), args);
        assert_eq!(
            first_roles.keys().copied().collect::<BTreeSet<_>>(),
            role_ids.into_iter().collect()
        );
        assert_eq!(
            first_roles
                .values()
                .copied()
                .chain([first_node_session])
                .collect::<BTreeSet<_>>()
                .len(),
            4
        );
        assert!(first_log.contains(&node_id.to_string()));
        assert!(!source.contains("session_id"));
        let (second_log, second_node_session, second_roles) = start(&binary, cwd.path(), args);
        assert_ne!(first_node_session, second_node_session);
        assert!(second_log.contains("节点甲"));
        for role in role_ids {
            assert_ne!(first_roles[&role], second_roles[&role]);
            assert_ne!(role, second_roles[&role]);
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        // Moving the role to another Node leaves its persistent target identity intact.
        let moved = source
            .replace(&node_id.to_string(), &Uuid::new_v4().to_string())
            .replace("节点甲", "节点乙");
        fs::write(&path, &moved).unwrap();
        let (log, _, roles) = start(&binary, cwd.path(), args);
        assert!(log.contains("节点乙"));
        assert_eq!(
            roles.keys().copied().collect::<BTreeSet<_>>(),
            role_ids.into_iter().collect()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), moved);
    }
}
