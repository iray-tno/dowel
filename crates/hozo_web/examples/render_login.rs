//! Renders the proposal's Login example end-to-end through
//! hozo_parser -> hozo_web and prints a standalone HTML page to stdout.
//! `cargo run --example render_login -p hozo_web > out.html`



const LOGIN_EXAMPLE: &str = r#"
import { View, Text, Button } from '@hozo/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">
        Welcome
      </Text>

      <Button className="mt-4 px-4 py-2">
        Continue
      </Button>
    </View>
  )
}
"#;

fn main() {
    let parsed = hozo_parser::parse_tsx(LOGIN_EXAMPLE);
    let output = hozo_web::lower(&parsed.roots[0].node, LOGIN_EXAMPLE, &hozo_ir::Theme::default());
    // hozo_web emits `className` (its real consumer is JSX/React); this
    // demo renders a plain standalone HTML page instead, so swap it back
    // to the attribute a browser actually matches CSS selectors against.
    let html = output.jsx.replace("className=", "class=");
    println!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Hozo: Login example</title>\n<style>\n{}\n</style>\n</head>\n<body>\n{}\n</body>\n</html>",
        output.css, html
    );
}
