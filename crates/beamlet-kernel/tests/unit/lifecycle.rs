use super::{Lifecycle, Ready};

/// 验证内核生命周期图无不可达状态或终态出口等结构问题.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn lifecycle_graph_is_valid() {
    let diagnostics = Lifecycle::<(), Ready>::schema().validate();
    assert!(diagnostics.is_empty(), "生命周期图无效: {diagnostics:?}");
}
