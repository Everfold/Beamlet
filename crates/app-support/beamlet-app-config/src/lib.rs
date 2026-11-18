//! 使用 confy 加载 RON, 首次创建时将注释放在对应字段上方.

use ron::ser::{PrettyConfig, path_meta::Field};
use ron::value::RawValue;
use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned, ser::Error};
use std::path::Path;

/// 应用配置及其首次写入的字段说明.
pub trait Config: Default + Serialize + DeserializeOwned {
    /// RON 字段路径及注释, 嵌套字段以点分隔.
    const COMMENTS: &[(&str, &str)];
}

/// 从 `path` 加载配置, 缺失时创建带注释的默认值. 返回配置或读写错误.
pub fn load<T: Config>(path: &Path) -> Result<T, confy::ConfyError> {
    confy::load_path::<Document<T>>(path).map(|document| document.0)
}

/// 读取时透传配置, 写入时使用 RON 的字段注释.
#[derive(Default, Deserialize)]
#[serde(transparent)]
struct Document<T>(T);

impl<T: Config> Serialize for Document<T> {
    /// 将带字段注释的 RON 交给 `serializer`, 返回序列化结果.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut pretty = PrettyConfig::default();
        let root = pretty.path_meta.insert(Field::empty());
        for &(path, comment) in T::COMMENTS {
            add_comment(root, path, comment);
        }
        let ron = ron::ser::to_string_pretty(&self.0, pretty).map_err(S::Error::custom)?;
        RawValue::from_ron(&ron)
            .map_err(S::Error::custom)?
            .serialize(serializer)
    }
}

/// 在 `parent` 下沿 `path` 添加 `comment`.
fn add_comment(parent: &mut Field, path: &str, comment: &str) {
    let (key, rest) = path.split_once('.').unwrap_or((path, ""));
    let insert = |fields: &mut ron::ser::path_meta::Fields| {
        let field = fields.field(key);
        if rest.is_empty() {
            field.with_doc(comment);
        } else {
            add_comment(field, rest, comment);
        }
    };
    if let Some(fields) = parent.fields_mut() {
        insert(fields);
    } else {
        parent.build_fields(insert);
    }
}
