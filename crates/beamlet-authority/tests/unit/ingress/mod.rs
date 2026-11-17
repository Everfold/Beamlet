pub(crate) use crate::{
    Request,
    engine::{Ingress, Reply},
};
/// 测试使用的有界结构化请求输入和一次性结果回复。
mod channel;
/// 当前模块关键行为与边界条件的单元测试，实现在 tests/unit 目录。
mod tests;
