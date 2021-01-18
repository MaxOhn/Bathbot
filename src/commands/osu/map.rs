use crate::{
    arguments::{Args, MapModArgs},
    bail,
    embeds::{EmbedData, MapEmbed},
    pagination::{MapPagination, Pagination},
    unwind_error,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        error::PPError,
        osu::{cached_message_extract, map_id_from_history, prepare_beatmap_file, MapIdType},
        MessageExt,
    },
    BotResult, Context, Error,
};

use chrono::Duration;
use image::{png::PngEncoder, ColorType, DynamicImage};
use plotters::prelude::*;
use rayon::prelude::*;
use rosu::model::{GameMode, GameMods};
use rosu_pp::{Beatmap, BeatmapExt};
use std::{cmp::Ordering, fs::File, sync::Arc};
use twilight_model::channel::Message;

const W: u32 = 590;
const H: u32 = 150;

#[command]
#[short_desc("Display a bunch of stats about a map(set)")]
#[long_desc(
    "Display stats about a beatmap. Mods can be specified.\n\
    If no map(set) is specified by either url or id, I will choose the last map \
    I can find in the embeds of this channel.\n\
    If the mapset is specified by id but there is some map with the same id, \
    I will choose the latter."
)]
#[usage("[map(set) url / map(set) id] [+mods]")]
#[example("2240404 +hddt", "https://osu.ppy.sh/beatmapsets/902425 +hr")]
#[aliases("m", "beatmap", "maps", "beatmaps", "mapinfo")]
async fn map(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = MapModArgs::new(args);
    let map_id = if let Some(id) = args.map_id {
        id
    } else if let Some(id) = ctx
        .cache
        .message_extract(msg.channel_id, cached_message_extract)
    {
        id
    } else {
        let msgs = match ctx.retrieve_channel_history(msg.channel_id).await {
            Ok(msgs) => msgs,
            Err(why) => {
                let _ = msg.error(&ctx, GENERAL_ISSUE).await;
                return Err(why.into());
            }
        };
        match map_id_from_history(msgs) {
            Some(id) => id,
            None => {
                let content = "No beatmap specified and none found in recent channel history. \
                    Try specifying a map(set) either by url to the map, \
                    or just by map(set) id.";
                return msg.error(&ctx, content).await;
            }
        }
    };
    let mods = match args.mods {
        Some(selection) => selection.mods(),
        None => GameMods::NoMod,
    };

    // Retrieving the beatmaps
    let (mapset_id, map_id) = match map_id {
        // If its given as map id, try to convert into mapset id
        MapIdType::Map(id) => {
            // Check if map is in DB
            match ctx.psql().get_beatmap(id).await {
                Ok(map) => (map.beatmapset_id, Some(id)),
                Err(_) => {
                    // If not in DB, request through API
                    match ctx.osu().beatmap().map_id(id).await {
                        Ok(Some(map)) => (map.beatmapset_id, Some(id)),
                        Ok(None) => (id, None),
                        Err(why) => {
                            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
                            return Err(why.into());
                        }
                    }
                }
            }
        }
        // If its already given as mapset id, do nothing
        MapIdType::Set(id) => (id, None),
    };
    // Request mapset through API
    let maps = match ctx.osu().beatmaps().mapset_id(mapset_id).await {
        Ok(mut maps) => {
            // For mania sort first by mania key, then star rating
            if maps.first().map(|map| map.mode).unwrap_or_default() == GameMode::MNA {
                maps.sort_unstable_by(|m1, m2| {
                    m1.diff_cs
                        .partial_cmp(&m2.diff_cs)
                        .unwrap_or(Ordering::Equal)
                        .then(m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal))
                })
            // For other mods just sort by star rating
            } else {
                maps.sort_unstable_by(|m1, m2| {
                    m1.stars.partial_cmp(&m2.stars).unwrap_or(Ordering::Equal)
                })
            }
            maps
        }
        Err(why) => {
            let _ = msg.error(&ctx, OSU_API_ISSUE).await;
            return Err(why.into());
        }
    };
    let map_idx = if let Some(first_map) = maps.first() {
        let first_map_id = map_id.unwrap_or(first_map.beatmap_id);
        maps.iter()
            .position(|map| map.beatmap_id == first_map_id)
            .unwrap_or(0)
    } else {
        let content = "API returned no map for this id";
        return msg.error(&ctx, content).await;
    };

    let map = &maps[map_idx];

    // Try creating the strain graph for the map
    let bg_fut = async {
        let url = format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            map.beatmapset_id
        );
        let res = reqwest::get(&url).await?.bytes().await?;
        Ok::<_, Error>(image::load_from_memory(res.as_ref())?.thumbnail_exact(W, H))
    };
    let graph = match tokio::join!(strain_values(map.beatmap_id, mods), bg_fut) {
        (Ok(strain_values), Ok(img)) => match graph(strain_values, img) {
            Ok(graph) => Some(graph),
            Err(why) => {
                unwind_error!(warn, why, "Error creating graph: {}");
                None
            }
        },
        (Err(why), _) => {
            unwind_error!(warn, why, "Error while creating oppai_values: {}");
            None
        }
        (_, Err(why)) => {
            unwind_error!(warn, why, "Error retrieving graph background: {}");
            None
        }
    };

    // Accumulate all necessary data
    let data_fut = MapEmbed::new(
        &maps[map_idx],
        mods,
        graph.is_none(),
        (map_idx + 1, maps.len()),
    );
    let data = match data_fut.await {
        Ok(data) => data,
        Err(why) => {
            let _ = msg.error(&ctx, GENERAL_ISSUE).await;
            return Err(why);
        }
    };

    // Sending the embed
    let embed = data.build().build()?;
    let m = ctx.http.create_message(msg.channel_id).embed(embed)?;
    let response = if let Some(ref graph) = graph {
        m.attachment("map_graph.png", graph.clone()).await?
    } else {
        m.await?
    };

    // Add missing maps to database
    match ctx.clients.psql.insert_beatmaps(&maps).await {
        Ok(n) if n < 2 => {}
        Ok(n) => info!("Added {} maps to DB", n),
        Err(why) => unwind_error!(warn, why, "Error while adding maps to DB: {}"),
    }

    // Skip pagination if too few entries
    if maps.len() < 2 {
        response.reaction_delete(&ctx, msg.author.id);
        return Ok(());
    }

    // Pagination
    let pagination = MapPagination::new(response, maps, mods, map_idx, graph.is_none());
    let owner = msg.author.id;
    tokio::spawn(async move {
        if let Err(why) = pagination.start(&ctx, owner, 60).await {
            unwind_error!(warn, why, "Pagination error (map): {}")
        }
    });
    Ok(())
}

async fn strain_values(map_id: u32, mods: GameMods) -> BotResult<Vec<(f32, f32)>> {
    let map_path = prepare_beatmap_file(map_id).await?;
    let file = File::open(map_path)?;
    let map = Beatmap::parse(file).map_err(PPError::from)?;
    let strains = map.strains(mods.bits());
    let section_len = strains.section_length;

    let strains = strains
        .strains
        .into_iter()
        .scan(0.0, |time, strain| {
            *time += section_len;

            Some((*time, strain))
        })
        .collect();

    Ok(strains)
}

fn graph(strains: Vec<(f32, f32)>, background: DynamicImage) -> BotResult<Vec<u8>> {
    static LEN: usize = W as usize * H as usize;

    let max_strain = strains
        .par_iter()
        .copied()
        .max_by(|(_, a), (_, b)| a.partial_cmp(&b).unwrap_or(Ordering::Equal))
        .map_or(0.0, |(_, s)| s);

    if max_strain <= std::f32::EPSILON {
        bail!("no non-zero strain point");
    }

    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(17)
            .build_cartesian_2d(0.0..strains.last().unwrap().0, 0.0..max_strain)?;

        // Take as line color whatever is represented least in the background
        let (r, g, b) = background
            .to_rgba8()
            .pixels()
            .par_bridge()
            .map(|pixel| (pixel[0] as u64, pixel[1] as u64, pixel[2] as u64))
            .reduce(|| (0, 0, 0), |sums, curr| sums.add(curr));
        let b = (b as f32 * 1.1) as u64;
        let line_color = match r.min(g).min(b) {
            min if min == r => &RED,
            min if min == g => &GREEN,
            min if min == b => &BLUE,
            _ => unreachable!(),
        };

        // Add background
        let elem: BitMapElement<_> = ((0.0, max_strain), background).into();
        chart.draw_series(std::iter::once(elem))?;

        // Mesh and labels
        let text_style = FontDesc::new(FontFamily::Serif, 11.0, FontStyle::Bold).color(line_color);
        chart
            .configure_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .set_all_tick_mark_size(3)
            .light_line_style(&BLACK.mix(0.0))
            .x_labels(10)
            .x_label_style(text_style)
            .x_label_formatter(&|timestamp| {
                if timestamp.abs() < f32::EPSILON {
                    return String::new();
                }

                let d = Duration::milliseconds(*timestamp as i64);
                let minutes = d.num_seconds() / 60;
                let seconds = d.num_seconds() % 60;

                format!("{}:{:0>2}", minutes, seconds)
            })
            .draw()?;

        // Draw line
        chart.draw_series(LineSeries::new(
            strains.into_iter().map(|(time, strain)| (time, strain)),
            line_color,
        ))?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PngEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;

    Ok(png_bytes)
}

trait TupleExt: Sized {
    fn add(self, other: (u64, u64, u64)) -> Self;
}

impl TupleExt for (u64, u64, u64) {
    fn add(self, other: (u64, u64, u64)) -> Self {
        (self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}
