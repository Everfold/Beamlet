use crate::Attempt;

/// 一次 Attempt 执行结束后的结果证据, 身份和结果类型由调用者定义.
///
/// # 泛型参数
///
/// - `O`：Operation 身份类型。
/// - `A`：Attempt 身份类型。
/// - `R`：Report 身份类型。
/// - `V`：报告业务结果类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Report<O, A, R, V> {
    /// 本次报告的身份。
    id: R,
    /// 已结束执行所对应的原始 Attempt。
    attempt: Attempt<O, A>,
    /// 业务执行结果；其成功或失败语义由调用方解释。
    result: V,
}

impl<O, A, R, V> Report<O, A, R, V> {
    /// 把已结束的 Attempt 与报告身份、业务结果组成 Report。
    ///
    /// # 参数
    ///
    /// - `id`：本次报告的身份，由调用方分配。
    /// - `attempt`：已经签发的执行授权，携带所属 Operation 和 Attempt 身份。
    /// - `result`：业务处理器产生的结果，不在此判断其是否满足请求。
    ///
    /// # 返回值
    ///
    /// 拥有原 Attempt 和业务结果的完整 Report，不判断业务结果是否成功。
    #[must_use]
    pub fn new(id: R, attempt: Attempt<O, A>, result: V) -> Self {
        Self {
            id,
            attempt,
            result,
        }
    }

    /// 借用报告身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// Report 身份的只读引用。
    pub fn id(&self) -> &R {
        &self.id
    }

    /// 借用授权或报告所属的 Operation 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 所属 Operation 身份的只读引用。
    pub fn operation_id(&self) -> &O {
        self.attempt.operation_id()
    }

    /// 借用报告对应的 Attempt 身份。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 已结束授权身份的只读引用。
    pub fn attempt_id(&self) -> &A {
        self.attempt.id()
    }

    /// 借用报告业务结果。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的共享引用。
    ///
    /// # 返回值
    ///
    /// 业务结果的只读引用，结果语义由调用方解释。
    pub fn result(&self) -> &V {
        &self.result
    }

    /// 消费完整报告并拆出所有字段。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 三元组 `(Report 身份, Attempt, 业务结果)` 的所有权。
    pub fn into_parts(self) -> (R, Attempt<O, A>, V) {
        (self.id, self.attempt, self.result)
    }

    /// 消费报告并取出业务结果。
    ///
    /// # 参数
    ///
    /// - `self`：当前 `Report<O, A, R, V>` 的值；调用后转移其所有权（Copy 类型按值复制）。
    ///
    /// # 返回值
    ///
    /// 结果的所有权；报告身份及授权随报告被消费。
    pub fn into_result(self) -> V {
        self.result
    }
}
