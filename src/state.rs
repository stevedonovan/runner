pub struct State {
    pub build_static: bool,
    pub optimize: bool,
    pub exe: bool,
    pub edition: String,
}

impl State {
    pub fn exe(is_static: bool, optimized: bool, edition: &str) -> State {
        State {
            build_static: is_static,
            optimize: optimized,
            exe: true,
            edition: edition.into(),
        }
    }

    pub fn dll(optimized: bool, edition: &str) -> State {
        State {
            build_static: false,
            optimize: optimized,
            exe: false,
            edition: edition.into(),
        }
    }
}
