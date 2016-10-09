use clap::{Arg, ArgMatches, App};
use regex::Regex;
use std::path::PathBuf;
use super::Config;

lazy_static! {
    static ref IMG_DIM_REGEX: Regex = Regex::new("^([:digit:]+)x([:digit:]+)$").unwrap();
}

fn is_img_dim(s: String) -> Result<(), String> {
    if IMG_DIM_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be 'WxH' where W and H are positive integers".to_string())
    }
}

fn is_positive_int(s: String) -> Result<(), String> {
    lazy_static! {
        static ref POSITIVE_INT_REGEX: Regex = Regex::new("^[:digit:]+$").unwrap();
    }
    if POSITIVE_INT_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be a positive integer".to_string())
    }
}

fn is_positive_float(s: String) -> Result<(), String> {
    lazy_static! {
        static ref POSITIVE_FLOAT_REGEX: Regex =
            Regex::new(r"^[:digit:]+\.[:digit:]+(e[+-]?[:digit:]+)?$").unwrap();
    }
    if POSITIVE_FLOAT_REGEX.is_match(&s) {
        Ok(())
    } else {
        Err("Value must be a positive number of the form 12.34e56".to_string())
    }
}

pub fn build_app() -> App<'static, 'static> {
    App::new("suptracer")
        .version("0.0.0")
        .author("Robin Kruppe <robin.kruppe@gmail.com>")
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
        .arg(Arg::with_name("outfile")
            .short("o")
            .long("out")
            .help("File name for output")
            .value_name("FILE")
            .required(false))
        .arg(Arg::with_name("sah-traversal-cost")
            .long("sah-tcost")
            .help("Relative cost of BVH traversal step compared to triangle intersection")
            .value_name("COST")
            .default_value("8.0")
            .validator(is_positive_float))
        .arg(Arg::with_name("input")
            .help("OBJ file to render")
            .value_name("FILE")
            .required(true)
            .index(1))
}

pub fn parse_matches(matches: ArgMatches) -> Config {
    let dim = matches.value_of("dimensions").unwrap();
    let dim_captures = IMG_DIM_REGEX.captures(dim).unwrap();
    let (width, height) = (dim_captures[1].parse().unwrap(), dim_captures[2].parse().unwrap());
    Config {
        input_file: matches.value_of("input").map(PathBuf::from).unwrap(),
        image_width: width,
        image_height: height,
        sah_buckets: matches.value_of("sah-buckets").unwrap().parse().unwrap(),
        sah_traversal_cost: matches.value_of("sah-traversal-cost").unwrap().parse().unwrap(),
    }
}