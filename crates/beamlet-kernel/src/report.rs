/// 从最终 Report 提取的关联信息, 尚需内核校验.
///
/// O 和 A 分别为 Operation 与 Attempt 的身份类型, 完整报告由外层持有.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportAssociation<O, A> {
    /// Report 所属的 Operation 身份.
    pub operation_id: O,
    /// Report 对应的 Attempt 身份.
    pub attempt_id: A,
}
