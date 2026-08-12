//! Renders the proposal's Login example end-to-end through
//! dowel_parser -> dowel_web and prints a standalone HTML page to stdout.
//! `cargo run --example render_login -p dowel_web > out.html`



const LOGIN_EXAMPLE: &str = r#"
import { View, Text, Button } from '@dowel/core'

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
    let parsed = dowel_parser::parse_tsx(LOGIN_EXAMPLE);
    let output = dowel_web::lower(&parsed.roots[0]);
    println!(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Dowel: Login example</title>\n<style>\n{}\n</style>\n</head>\n<body>\n{}\n</body>\n</html>",
        output.css, output.html
    );
}
