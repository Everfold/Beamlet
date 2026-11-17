use super::{Lifecycle, Ready};

/// 复用库的结构检查, 防止状态不可达或终态被意外添加出口.
#[test]
fn lifecycle_graph_is_valid() {
    let diagnostics = Lifecycle::<(), Ready>::schema().validate();
    assert!(diagnostics.is_empty(), "生命周期图无效: {diagnostics:?}");
}
