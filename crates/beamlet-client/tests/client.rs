/// Client 发现去重, 期限与取消行为的公开接口测试.
#[path = "cases/discovery.rs"]
mod discovery;
/// Client 并发提交, 回复关联和错误分类的公开接口测试.
#[path = "cases/requests.rs"]
mod requests;
/// 测试所需的受控传输, 身份或工作项辅助代码.
mod support;
/// Client 构造参数和发送前命令校验的公开接口测试.
#[path = "cases/validation.rs"]
mod validation;
