//! 通过真实启动入口验证持久节点身份与一次性会话的边界.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};
use uuid::Uuid;

#[derive(Default, Deserialize, Serialize)]
struct StoredIdentity {
    node_id: Uuid,
}

fn start(binary: &Path, working_directory: &Path) -> (String, Uuid) {
    let output = Command::new(binary)
        .current_dir(working_directory)
        .output()
        .unwrap();
    // 配置使用不可解析的地址, 在连接网络前退出; 身份初始化必须已完成.
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("连接 NATS 失败"), "{stderr}");
    assert!(output.stdout.is_empty(), "日志统一输出至 stderr");
    assert!(stderr.contains("ERROR"), "{stderr}");
    assert!(stderr.contains("event=\"node.failed\""), "{stderr}");
    let startup = stderr
        .lines()
        .find(|line| line.contains("event=\"node.starting\""))
        .expect("必须输出节点启动事件");
    assert!(startup.contains("INFO"), "{startup}");
    let session = startup
        .split_whitespace()
        .find_map(|field| field.strip_prefix("session_id="))
        .expect("启动日志必须包含会话 UUID")
        .parse::<Uuid>()
        .unwrap();
    assert!(!session.is_nil());
    (startup.to_owned(), session)
}

#[test]
fn restart_preserves_node_id_and_label_but_renews_session() {
    for role in ["authority", "worker"] {
        let directory = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        fs::write(cwd.path().join("node.ron"), "invalid cwd config").unwrap();
        let binary = directory
            .path()
            .join(format!("beamlet-node{}", std::env::consts::EXE_SUFFIX));
        fs::copy(env!("CARGO_BIN_EXE_beamlet-node"), &binary).unwrap();
        let path = directory.path().join("node.ron");
        let expected_node_id = Uuid::new_v4();
        fs::write(
            &path,
            format!(
                r#"(node_id: "{expected_node_id}", role: {role}, label: Some("节点甲"), nats_url: "nats://[invalid")"#
            ),
        )
        .unwrap();

        let (first_log, first_session) = start(&binary, cwd.path());
        let content = fs::read_to_string(&path).unwrap();
        let config: StoredIdentity = confy::load_path(&path).unwrap();
        let node_id = config.node_id;
        assert_eq!(node_id, expected_node_id);
        assert!(
            first_log.contains(&format!("node_id={node_id}")),
            "{first_log}"
        );
        assert!(
            first_log.contains(&format!("role=\"{role}\"")),
            "{first_log}"
        );
        assert!(first_log.contains("节点甲"));
        assert_ne!(node_id, first_session);
        assert!(!content.contains("session_id"));
        assert!(!content.contains(&first_session.to_string()));

        let (second_log, second_session) = start(&binary, cwd.path());
        assert!(second_log.contains(&node_id.to_string()));
        assert!(second_log.contains("节点甲"));
        assert_ne!(first_session, second_session);
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        fs::write(&path, content.replace("节点甲", "节点乙")).unwrap();
        let (renamed_log, renamed_session) = start(&binary, cwd.path());
        assert!(renamed_log.contains("节点乙"));
        assert!(renamed_log.contains(&node_id.to_string()));
        assert_ne!(second_session, renamed_session);
    }
}
