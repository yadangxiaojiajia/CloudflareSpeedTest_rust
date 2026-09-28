//! 前端资源：编译时内嵌，运行时零外部依赖

const INDEX_HTML: &str = include_str!("../../frontend/index.html");
const APP_JS: &str = include_str!("../../frontend/app.js");
const STYLE_CSS: &str = include_str!("../../frontend/style.css");

pub fn index_html() -> &'static str {
    INDEX_HTML
}

pub fn app_js() -> &'static str {
    APP_JS
}

pub fn style_css() -> &'static str {
    STYLE_CSS
}
