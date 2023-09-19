use std::str::from_utf8 as str_from_utf8;

use skia_safe::Path;

use crate::error::SvgError;

pub(crate) struct Svg {
    pub(crate) view_box_w: i32,
    pub(crate) view_box_h: i32,
    pub(crate) path: Path,
}

impl Svg {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, SvgError> {
        const VIEW_BOX_NEEDLE: &str = " viewBox=\"";
        const PATH_NEEDLE: &str = " d=\"";

        let svg = str_from_utf8(bytes)?;

        let start =
            svg.find(VIEW_BOX_NEEDLE).ok_or(SvgError::MissingViewBox)? + VIEW_BOX_NEEDLE.len();
        let end = svg[start..].find('"').ok_or(SvgError::MissingViewBox)?;
        let mut iter = svg[start..start + end].split_ascii_whitespace().skip(2);

        let view_box_w = iter
            .next()
            .ok_or(SvgError::MissingViewBoxW)?
            .parse()
            .map_err(|_| SvgError::ParseViewBox)?;

        let view_box_h = iter
            .next()
            .ok_or(SvgError::MissingViewBoxH)?
            .parse()
            .map_err(|_| SvgError::ParseViewBox)?;

        let start = svg.find(PATH_NEEDLE).ok_or(SvgError::MissingPath)? + PATH_NEEDLE.len();
        let end = svg[start..].find('"').ok_or(SvgError::MissingPath)?;
        let path = Path::from_svg(&svg[start..start + end]).ok_or(SvgError::CreatePath)?;

        Ok(Self {
            view_box_h,
            view_box_w,
            path,
        })
    }
}
