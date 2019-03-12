pub fn split_balanced(s: &str, split_character: char) -> Vec<&str> {
    s.split(balanced(split_character)).collect::<Vec<_>>()
}

fn balanced(character_to_match: char) -> impl FnMut(char) -> bool {
    let mut num_parens = 0;
    move |c| match c {
        '{' => {
            num_parens += 1;
            false
        }
        '}' => {
            num_parens -= 1;
            false
        }
        other => num_parens == 0 && other == character_to_match,
    }
}
