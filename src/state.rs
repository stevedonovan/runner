use lapp::Args;

pub struct State {
    pub build_static: bool,
    pub optimize: bool,
    pub exe: bool,
    pub edition: String,
    pub verbose: bool,
    pub simplify: bool,
    pub libc: bool,
    pub features: Vec<String>,
    pub link: Option<String>,
    pub cfg: Vec<String>,
    pub externs: Vec<String>,
}

impl State {
    pub fn exe(is_static: bool, optimized: bool, args: &Args) -> State {
        Self::make_state(is_static, optimized, true, args)
    }

    fn make_state(build_static: bool, optimize: bool, exe: bool, args: &Args) -> State {
        State {
            build_static,
            optimize,
            exe,
            edition: args.get_string("edition"),
            verbose: args.get_bool("verbose"),
            simplify: !args.get_bool("no-simplify"),
            libc: args.get_bool("libc"),
            features: args.get_strings("features"),
            link: args.get_string_result("link").ok(),
            cfg: args.get_strings("cfg"),
            externs: args.get_strings("extern"),
        }
    }

    pub fn dll(optimized: bool, args: &Args) -> State {
        Self::make_state(false, optimized, false, args)
    }
}
