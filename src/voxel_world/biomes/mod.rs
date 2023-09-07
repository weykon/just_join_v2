use ndshape::{ConstShape, ConstShape2u32, ConstShape3u32};
use noise::{
    core::worley::{distance_functions::euclidean, ReturnType},
    utils::NoiseMapBuilder,
    Worley,
};

use crate::{CHUNK_SIZE, CHUNK_SIZE_U32};

use self::{
    basic_land::BasicLandBiomes, bule_land::BuleLandBoimes, dry_land::DryLandBiomes,
    sand_land::SandLandBiomes, snow_land::SnowLandBiomes,
};

use super::{chunk::ChunkKey, voxel::Voxel};

pub mod basic_land;
pub mod bule_land;
pub mod dry_land;
pub mod sand_land;
pub mod snow_land;

pub type SampleShape = ConstShape3u32<CHUNK_SIZE_U32, CHUNK_SIZE_U32, CHUNK_SIZE_U32>;
pub type PanleShap = ConstShape2u32<CHUNK_SIZE_U32, CHUNK_SIZE_U32>;

// 处理 生物群落
pub fn biomes_generate(
    chunk_key: ChunkKey,
    seed: i32,
    suface_index: Vec<u32>,
    voxels: &mut Vec<Voxel>,
) {
    if suface_index.len() == 0 {
        return;
    }
    // 生成噪声
    let noise = biomes_noise(chunk_key, seed);

    for index in suface_index {
        // 由噪声生产的特征值
        let [x, _, z] = SampleShape::delinearize(index);
        let index_2d = PanleShap::linearize([x, z]);
        let atrr = noise[index_2d as usize];
        let generator = get_generator_by_atrr(atrr);
        generator.gen_land(chunk_key.clone(), voxels, index, index_2d);
        // fixme: 这里要记录对于其他方块的影响
    }
}

// 获取不同的生成器
fn get_generator_by_atrr(data: f32) -> Box<dyn BiomesGenerator> {
    if data < 0.1 {
        return BasicLandBiomes.into_boxed_generator();
    } else if data < 0.4 {
        return DryLandBiomes.into_boxed_generator();
    } else if data < 0.6 {
        return SnowLandBiomes.into_boxed_generator();
    } else if data < 0.8 {
        return SandLandBiomes.into_boxed_generator();
    } else {
        return BuleLandBoimes.into_boxed_generator();
    }
}

pub fn biomes_noise(chunk_key: ChunkKey, seed: i32) -> Vec<f32> {
    let noise = Worley::new(seed as u32)
        .set_distance_function(euclidean)
        .set_return_type(ReturnType::Value)
        .set_frequency(0.008);

    let x_offset = (chunk_key.0.x * CHUNK_SIZE) as f64;
    let z_offset = (chunk_key.0.z * CHUNK_SIZE) as f64;

    noise::utils::PlaneMapBuilder::<_, 2>::new(noise)
        .set_size(CHUNK_SIZE as usize, CHUNK_SIZE as usize)
        .set_x_bounds(x_offset, x_offset + CHUNK_SIZE as f64)
        .set_y_bounds(z_offset, z_offset + CHUNK_SIZE as f64)
        .build()
        .into_iter()
        .map(|x| x as f32)
        .collect()
}

pub trait BiomesGenerator: 'static + Sync + Send {
    fn gen_land_with_info(
        &self,
        chunk_key: ChunkKey,
        voxels: &mut Vec<Voxel>,
        chunk_index: u32,
        plane_index: u32,
        height: f32,
        xyz: [u32; 3],
    );

    fn gen_land(
        &self,
        chunk_key: ChunkKey,
        voxels: &mut Vec<Voxel>,
        chunk_index: u32,
        plane_index: u32,
    ) {
        let base_y: f32 = (chunk_key.0.y * CHUNK_SIZE) as f32;
        let [x, y, z] = SampleShape::delinearize(chunk_index);
        let height = base_y + y as f32;
        self.gen_land_with_info(
            chunk_key,
            voxels,
            chunk_index,
            plane_index,
            height,
            [x, y, z],
        );
    }
}

pub trait IntoBoxedTerrainGenerator: BiomesGenerator + Sized {
    fn into_boxed_generator(self) -> Box<Self>;
}

impl<T: BiomesGenerator> IntoBoxedTerrainGenerator for T {
    fn into_boxed_generator(self) -> Box<Self> {
        Box::new(self)
    }
}
// 海平面
pub const SEE_LEVEL: f32 = -60. + 76.;
// 山峰线
pub const MOUNTAIN_LEVEL: f32 = -60. + 100.;
// 雪线
pub const SNOW_LEVEL: f32 = -60. + 100.;
