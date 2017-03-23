use super::{Config, RenderKind};
use clap::{Arg, ArgMatches, App};
use regex::Regex;
use std::path::PathBuf;
use std::str::FromStr;

lazy_static! {
    static ref IMG_DIM_REGEX: Regex = Regex::new("^([:digit:]+)x([:digit:]+)$").unwrap();
    static ref POSITIVE_INT_REGEX: Regex = Regex::new("^[:digit:]+$").unwrap();
    static ref POSITIVE_FLOAT_REGEX: Regex = Regex::new(r"^[:digit:]+\.[:digit:]+$").unwrap();
}

fn is_img_dim(s: String) -> Result<(), String> {
    if IMG_DIM_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be 'WxH' where W and H are positive integers".to_string())
    }
}

fn is_positive_int(s: String) -> Result<(), String> {
    if POSITIVE_INT_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be a positive integer".to_string())
    }
}

fn is_positive_float(s: String) -> Result<(), String> {
    if POSITIVE_FLOAT_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be a positive number of the form 12.34".to_string())
    }
}

pub fn build_app() -> App<'static, 'static> {
    App::new("suptracer")
        .version("0.0.0")
        .author(crate_authors!())
        .about("Approximately the simplest useful path tracer")
        .arg(Arg::with_name("dimensions")
                 .short("d")
                 .long("dim")
                 .help("the size of the image to render")
                 .value_name("DIM")
                 .default_value("1280x720")
                 .validator(is_img_dim))
        .arg(Arg::with_name("sah-buckets")
                 .short("b")
                 .long("buckets")
                 .help("Number of buckets to use in SAH-guided BVH construction")
                 .value_name("N")
                 .default_value("16")
                 .validator(is_positive_int))
        .arg(Arg::with_name("output")
                 .short("o")
                 .long("out")
                 .help("File name for output")
                 .value_name("FILE")
                 .required(false))
        .arg(Arg::with_name("sah-traversal-cost")
                 .long("sah-tcost")
                 .help("Relative cost of BVH traversal step compared to triangle intersection")
                 .value_name("COST")
                 .default_value("1.0")
                 .validator(is_positive_float))
        .arg(Arg::with_name("input")
                 .help("OBJ file to render")
                 .value_name("FILE")
                 .required(true)
                 .index(1))
        .arg(Arg::with_name("num-threads")
                 .short("j")
                 .help("Number of threads to use")
                 .value_name("N")
                 .required(false)
                 .validator(is_positive_int))
        .arg(Arg::with_name("render-kind")
                 .short("k")
                 .long("kind")
                 .help("Kind of render to create")
                 .default_value("depth")
                 .possible_values(&["depth", "heat"]))
}

pub fn parse_matches(matches: ArgMatches) -> Config {
    fn parse_arg<T: FromStr>(matches: &ArgMatches, key: &str) -> Option<T> {
        matches.value_of(key).and_then(|s| s.parse().ok())
    }

    let input_file = matches.value_of_os("input").map(PathBuf::from).unwrap();
    let output_file = matches.value_of_os("output")
        .map(PathBuf::from)
        .unwrap_or(input_file.with_extension("bmp"));

    let dim = matches.value_of("dimensions").unwrap();
    let dim_captures = IMG_DIM_REGEX.captures(dim).unwrap();
    Config {
        input_file,
        output_file,
        image_width: dim_captures[1].parse().unwrap(),
        image_height: dim_captures[2].parse().unwrap(),
        sah_buckets: parse_arg(&matches, "sah-buckets").unwrap(),
        sah_traversal_cost: parse_arg(&matches, "sah-traversal-cost").unwrap(),
        num_threads: parse_arg(&matches, "num-threads"),
        render_kind: match matches.value_of("render-kind") {
            Some("depth") => RenderKind::Depthmap,
            Some("heat") => RenderKind::Heatmap,
            other => panic!("BUG: unhandled render-kind {:?}", other),
        },
    }
}
