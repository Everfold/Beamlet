/// 有独立身份的不可变请求, 请求内容由调用者定义.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Operation<O, P> {
    id: O,
    request: P,
}

impl<O, P> Operation<O, P> {
    /// 接收调用者生成的唯一身份及请求内容.
    #[must_use]
    pub fn new(id: O, request: P) -> Self {
        Self { id, request }
    }

    /// 借用 Operation 身份.
    pub fn id(&self) -> &O {
        &self.id
    }

    /// 借用原始请求, 生命周期转换不会修改它.
    pub fn request(&self) -> &P {
        &self.request
    }
}
