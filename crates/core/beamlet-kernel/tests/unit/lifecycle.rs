use super::{Lifecycle, Ready};

/// 验证内核生命周期图无不可达状态或终态出口等结构问题.
#[test]
fn lifecycle_graph_is_valid() {
    let diagnostics = Lifecycle::<(), Ready>::schema().validate();
    assert!(diagnostics.is_empty(), "生命周期图无效: {diagnostics:?}");
}
