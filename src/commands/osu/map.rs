use crate::{
    arguments::{Args, MapModArgs, ID},
    embeds::{EmbedData, MapEmbed},
    pagination::{MapPagination, Pagination},
    pp::roppai::Oppai,
    util::{
        constants::{GENERAL_ISSUE, OSU_API_ISSUE},
        osu::prepare_beatmap_file,
        MessageExt,
    },
    BotResult, Context,
};

use chrono::Duration;
use image::{png::PNGEncoder, ColorType, DynamicImage};
use plotters::prelude::*;
use rosu::{
    backend::requests::BeatmapRequest,
    models::{GameMode, GameMods},
};
use std::sync::Arc;
use twilight::model::channel::Message;

const W: u32 = 590;
const H: u32 = 150;

#[command]
#[short_desc("Display a bunch of stats about a map(set)")]
#[long_desc(
    "Display stats about a beatmap. Mods can be specified.\n\
    If no map(set) is specified by either url or id, I will choose the last map \
    I can find in my embeds of this channel.\n\
    If the mapset is specified by id but there is some map with the same id, \
    I will choose the latter."
)]
#[usage("[map(set) url / map(set) id] [+mods]")]
#[example("2240404 +hddt")]
#[example("https://osu.ppy.sh/beatmapsets/902425 +hr")]
#[aliases("beatmap", "maps", "beatmaps", "mapinfo")]
async fn map(ctx: Arc<Context>, msg: &Message, args: Args) -> BotResult<()> {
    let args = MapModArgs::new(args);
    let map_id = if let Some(id) = args.map_id {
        id
    } else {
        let msgs = msg
            .channel_id
            .messages(ctx, |retriever| retriever.limit(50))
            .await?;
        match discord::map_id_from_history(msgs, &ctx.cache).await {
            Some(id) => ID::Map(id),
            None => {
                let content =
                        "No beatmap specified and none found in recent channel history. \
                        Try specifying a map(set) either by url to the map, or just by map(set) id.";
                msg.respond(&ctx, content).await;
                return Ok(());
            }
        }
    };
    let mods = match args.mods {
        Some((mods, _)) => mods,
        None => GameMods::NoMod,
    };

    // Retrieving the beatmaps
    let (maps, first_map_id) = {
        let data = ctx.data.read().await;
        let osu = data.get::<Osu>().unwrap();
        let (mapset_id, map_id) = match map_id {
            // If its given as map id, try to convert into mapset id
            ID::Map(id) => {
                // Check if map is in DB
                let mysql = data.get::<MySQL>().unwrap();
                match mysql.get_beatmap(id).await {
                    Ok(map) => (map.beatmapset_id, Some(id)),
                    Err(_) => {
                        // If not in DB, request through API
                        let map_req = BeatmapRequest::new().map_id(id);
                        match map_req.queue_single(&osu).await {
                            Ok(Some(map)) => (map.beatmapset_id, Some(id)),
                            Ok(None) => (id, None),
                            Err(why) => {
                                msg.respond(&ctx, OSU_API_ISSUE).await?;
                                return Err(why.into());
                            }
                        }
                    }
                }
            }
            // If its already given as mapset id, do nothing
            ID::Set(id) => (id, None),
        };
        // Request mapset through API
        let map_req = BeatmapRequest::new().mapset_id(mapset_id);
        let maps = match map_req.queue(&osu).await {
            Ok(mut maps) => {
                // For mania sort first by mania key, then star rating
                if maps.first().map(|map| map.mode).unwrap_or_default() == GameMode::MNA {
                    maps.sort_by(|m1, m2| {
                        m1.diff_cs
                            .partial_cmp(&m2.diff_cs)
                            .unwrap_or_else(|| std::cmp::Ordering::Equal)
                            .then(
                                m1.stars
                                    .partial_cmp(&m2.stars)
                                    .unwrap_or_else(|| std::cmp::Ordering::Equal),
                            )
                    })
                // For other mods just sort by star rating
                } else {
                    maps.sort_by(|m1, m2| {
                        m1.stars
                            .partial_cmp(&m2.stars)
                            .unwrap_or_else(|| std::cmp::Ordering::Equal)
                    })
                }
                maps
            }
            Err(why) => {
                msg.respond(&ctx, OSU_API_ISSUE).await?;
                return Err(why.into());
            }
        };
        if maps.is_empty() {
            msg.respond(&ctx, "API returned no map for this id").await?;
            return Ok(());
        }
        let first_map_id = map_id.unwrap_or_else(|| maps.first().unwrap().beatmap_id);
        (maps, first_map_id)
    };

    let map_idx = maps
        .iter()
        .position(|map| map.beatmap_id == first_map_id)
        .unwrap();
    let map = &maps[map_idx];
    // Try creating the strain graph for the map (only STD & TKO)
    let graph = match map.mode {
        GameMode::STD | GameMode::TKO => {
            let (oppai_values, img) = tokio::join!(oppai_values(map.beatmap_id, mods), async {
                let url = format!(
                    "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
                    map.beatmapset_id
                );
                let res = reqwest::get(&url).await?.bytes().await?;
                Ok::<_, Error>(image::load_from_memory(res.as_ref())?.thumbnail_exact(W, H))
            });
            if let Err(why) = oppai_values {
                warn!("Error while creating oppai_values: {}", why);
                None
            } else if let Err(why) = img {
                warn!("Error retrieving graph background: {}", why);
                None
            } else {
                let graph = graph(oppai_values?, img?);
                match graph {
                    Ok(graph) => Some(graph),
                    Err(why) => {
                        warn!("Error creating graph: {}", why);
                        None
                    }
                }
            }
        }
        GameMode::MNA | GameMode::CTB => None,
    };

    // Accumulate all necessary data
    let data = match MapEmbed::new(
        &maps[map_idx],
        mods,
        graph.is_none(),
        (map_idx + 1, maps.len()),
        Arc::clone(&ctx.data),
    )
    .await
    {
        Ok(data) => data,
        Err(why) => {
            msg.respond(&ctx, GENERAL_ISSUE).await?;
            return Err(why.into());
        }
    };

    // Sending the embed
    let resp = msg
        .channel_id
        .send_message(ctx, |m| {
            if graph.is_some() {
                let bytes = graph.as_deref().unwrap();
                m.add_file((bytes, "map_graph.png"));
            }
            m.embed(|e| data.build(e))
        })
        .await?;

    // Add missing maps to database
    {
        let data = ctx.data.read().await;
        let mysql = data.get::<MySQL>().unwrap();
        let len = maps.len();
        match mysql.insert_beatmaps(&maps).await {
            Ok(_) if len == 1 => {}
            Ok(_) => info!("Added {} maps to DB", len),
            Err(why) => warn!("Error while adding maps to DB: {}", why),
        }
    }

    // Skip pagination if too few entries
    if maps.len() < 2 {
        resp.reaction_delete(ctx, msg.author.id).await;
        return Ok(());
    }

    // Pagination
    let pagination = MapPagination::new(
        ctx,
        resp,
        msg.author.id,
        maps,
        mods,
        map_idx,
        graph.is_none(),
    )
    .await;
    let cache = Arc::clone(&ctx.cache);
    let http = Arc::clone(&ctx.http);
    tokio::spawn(async move {
        if let Err(why) = pagination.start(cache, http).await {
            warn!("Pagination error: {}", why)
        }
    });
    Ok(())
}

async fn oppai_values(map_id: u32, mods: GameMods) -> Result<(Vec<u32>, Vec<f32>), Error> {
    // Prepare oppai
    let map_path = prepare_beatmap_file(map_id).await?;
    let mut oppai = Oppai::new();
    oppai.set_mods(mods.bits()).calculate(&map_path)?;
    const MAX_COUNT: usize = 1000;
    let object_count = oppai.get_object_count();
    let mods = oppai.get_mods();
    let time_coeff = if mods.contains(GameMods::DoubleTime) {
        2.0 / 3.0
    } else if mods.contains(GameMods::HalfTime) {
        1.5
    } else {
        1.0
    };
    let mut time = Vec::with_capacity(object_count.min(MAX_COUNT + 1));
    let mut strain = Vec::with_capacity(object_count.min(MAX_COUNT + 1));
    let no_skip = object_count <= MAX_COUNT;
    let ratio = object_count as f32 / MAX_COUNT as f32;
    let mut counter = 0.0;
    let mut next_idx = 0;
    for i in 0..object_count {
        if no_skip || i == next_idx {
            time.push((oppai.get_time_at(i) as f32 * time_coeff) as u32);
            strain.push(oppai.get_strain_at(i, 0) + oppai.get_strain_at(i, 1));
            counter += ratio;
            next_idx = counter as usize;
        }
    }
    Ok((time, strain))
}

fn graph(oppai_values: (Vec<u32>, Vec<f32>), background: DynamicImage) -> Result<Vec<u8>, Error> {
    static LEN: usize = W as usize * H as usize;
    let (time, strain) = oppai_values;
    let max_strain = strain
        .iter()
        .fold(0.0, |max, &next| if next > max { next } else { max });
    let mut buf = vec![0; LEN * 3]; // PIXEL_SIZE = 3

    {
        let root = BitMapBackend::with_buffer(&mut buf, (W, H)).into_drawing_area();
        root.fill(&WHITE)?;
        let mut chart = ChartBuilder::on(&root)
            .x_label_area_size(17)
            .build_ranged(0..*time.last().unwrap(), 0.0..max_strain)?;

        // Take as line color whatever is represented least in the background
        let (r, g, b) =
            background
                .to_rgba()
                .enumerate_pixels()
                .fold((0, 0, 0), |(r, g, b), pixel| {
                    (
                        r + pixel.2[0] as u64,
                        g + pixel.2[1] as u64,
                        b + pixel.2[2] as u64,
                    )
                });
        let b = (b as f32 * 1.1) as u64;
        let line_color = match r.min(g).min(b) {
            min if min == r => &RED,
            min if min == g => &GREEN,
            min if min == b => &BLUE,
            _ => unreachable!(),
        };

        // Add background
        let elem: BitMapElement<_> = ((0, max_strain), background).into();
        chart.draw_series(std::iter::once(elem))?;

        // Mesh and labels
        let text_style = FontDesc::new(FontFamily::Serif, 11.0, FontStyle::Bold).color(line_color);
        chart
            .configure_mesh()
            .disable_y_mesh()
            .disable_y_axis()
            .set_all_tick_mark_size(3)
            .line_style_2(&BLACK.mix(0.0))
            .x_labels(10)
            .x_label_formatter(&|timestamp| {
                if *timestamp == 0 {
                    return String::new();
                }
                let d = Duration::milliseconds(*timestamp as i64);
                let minutes = d.num_seconds() / 60;
                let seconds = d.num_seconds() % 60;
                format!("{}:{:0>2}", minutes, seconds)
            })
            .x_label_style(text_style)
            .draw()?;

        // Draw line
        chart.draw_series(LineSeries::new(
            strain.into_iter().enumerate().map(|(i, x)| (time[i], x)),
            line_color,
        ))?;
    }

    // Encode buf to png
    let mut png_bytes: Vec<u8> = Vec::with_capacity(LEN);
    let png_encoder = PNGEncoder::new(&mut png_bytes);
    png_encoder.encode(&buf, W, H, ColorType::Rgb8)?;
    Ok(png_bytes)
}
