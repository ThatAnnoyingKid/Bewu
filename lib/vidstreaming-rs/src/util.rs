use std::borrow::Cow;

/// Try to make an input a https url
pub(crate) fn make_https(url: &str) -> Cow<'_, str> {
    if url.starts_with("//") {
        return format!("https:{url}").into();
    }

    url.into()
}
