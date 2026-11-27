// Default the display name when none was given (empty prompt, or --silent): title-case
// the kebab-cased project name, so `--name my-app` shows as "My App". Pass
// `-d app_name="Catchy TV"` to keep capitals the title-casing would lose.
if !variable::is_set("app_name") || variable::get("app_name") == "" {
    let words = [];
    for word in variable::get("project-name").split("-") {
        if word.len() > 0 {
            words.push(word.sub_string(0, 1).to_upper() + word.sub_string(1));
        }
    }
    variable::set("app_name", words.reduce(|name, word| if name == () { word } else { name + " " + word }));
}
