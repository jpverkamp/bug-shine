use noise::NoiseFn;
use rand::{seq::SliceRandom, Rng};

use crate::constants::*;

pub struct World {
    random: rand::rngs::ThreadRng,
    tiles: [[Tile; WIDTH as usize]; HEIGHT as usize],
    active: Vec<(usize, usize)>,
    winner: Option<u8>,
    roots: [(usize, usize); SPECIES],
    frame_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tile {
    Empty,
    Wall,
    Bug(u8, u8),
}

impl World {
    /// Create a new `World` instance that can draw a moving box.
    pub fn new() -> Self {
        let mut random = rand::thread_rng();
        let mut tiles = [[Tile::Empty; WIDTH as usize]; HEIGHT as usize];
        let mut active = Vec::new();
        let mut roots = [(0, 0); SPECIES];

        // Start with some NOISE
        let perlin = noise::Perlin::new(random.gen_range(0..1000));
        for x in 0..WIDTH {
            for y in 0..HEIGHT {
                let value = perlin.get([x as f64 / 100.0, y as f64 / 100.0, 0.0]);
                if value > 0.25 {
                    tiles[y as usize][x as usize] = Tile::Wall;
                }
            }
        }

        // Add some bugs, can't be on walls
        for id in 0..SPECIES {
            loop {
                let x = (WIDTH as f32 * rand::random::<f32>()) as usize;
                let y = (HEIGHT as f32 * rand::random::<f32>()) as usize;

                if tiles[y][x] != Tile::Empty {
                    continue;
                }

                tiles[y][x] = Tile::Bug(id as u8, MAX_AGE);
                active.push((x, y));
                roots[id] = (x, y);

                break;
            }
        }

        Self {
            random,
            tiles,
            active,
            winner: None,
            roots,
            frame_count: 0,
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.winner.is_some()
    }

    pub fn winner(&self) -> Option<u8> {
        self.winner
    }

    /// Update the `World` internal state; bounce the box around the screen.
    pub fn update(&mut self) {
        self.frame_count += 1;
        self.active.shuffle(&mut self.random);

        let mut next_active = Vec::new();

        let mut counts = [0; SPECIES];
        let mut actives = [0; SPECIES];

        // Expand active bugs
        for (x, y) in &self.active {
            if self.random.gen_range(0.0..1.0) < SKIP_CHANCE {
                next_active.push((*x, *y));
                continue;
            } else if self.random.gen_range(0.0..1.0) < DEACTIVE_CHANCE {
                continue;
            }

            if let Tile::Bug(id, _) = self.tiles[*y][*x] {
                counts[id as usize] += 1;

                for dx in -1..2 {
                    for dy in -1..2 {
                        if dx != 0 && dy != 0 {
                            continue;
                        }

                        let nx = *x as isize + dx;
                        let ny = *y as isize + dy;

                        if nx < 0 || ny < 0 || nx >= WIDTH || ny >= HEIGHT {
                            continue;
                        }

                        // if self.random.gen_range(0.0..1.0) > 0.8 {
                        //     // next_active.push((nx as usize, ny as usize));
                        //     continue;
                        // }

                        match self.tiles[ny as usize][nx as usize] {
                            Tile::Empty => {
                                self.tiles[ny as usize][nx as usize] = Tile::Bug(id, MAX_AGE);
                                next_active.push((nx as usize, ny as usize));
                            }
                            Tile::Bug(_, other_age) if other_age == 0 => {
                                self.tiles[ny as usize][nx as usize] = Tile::Bug(id, MAX_AGE);
                                next_active.push((nx as usize, ny as usize));
                            }
                            Tile::Wall => {}
                            Tile::Bug(_, _) => {}
                        }
                    }
                }
            }
        }

        // Age all bugs
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                if let Tile::Bug(id, age) = self.tiles[y as usize][x as usize] {
                    if age == 0 {
                        self.tiles[y as usize][x as usize] = Tile::Bug(id, 0);
                    } else {
                        self.tiles[y as usize][x as usize] = Tile::Bug(id, age - 1);
                        actives[id as usize] += 1;
                    }
                }
            }
        }

        // Reactivate at the root any bugs that aren't
        for id in 0..SPECIES {
            if counts[id] > 0 && self.random.gen_range(0.0..1.0) < PULSE_CHANCE {
                for _ in 0..100 {
                    // Root is still active
                    let (x, y) = self.roots[id];
                    if let Tile::Bug(root_id, _) = self.tiles[y][x] {
                        if id == root_id as usize {
                            self.tiles[y][x] = Tile::Bug(root_id, MAX_AGE);
                            next_active.push((x, y));
                            break;
                        }
                    }

                    // Root was taken over, find another one
                    for _ in 0..100 {
                        let x = (WIDTH as f32 * rand::random::<f32>()) as usize;
                        let y = (HEIGHT as f32 * rand::random::<f32>()) as usize;

                        if let Tile::Bug(root_id, _) = self.tiles[y][x] {
                            if id == root_id as usize {
                                self.tiles[y][x] = Tile::Bug(root_id, MAX_AGE);
                                next_active.push((x, y));
                                self.roots[id] = (x, y);
                                break;
                            }
                        }
                    }
                }
            }
        }

        if counts.iter().filter(|&count| *count > 0).count() == 1 {
            self.winner = Some(counts.iter().position(|&count| count > 0).unwrap() as u8);
        }

        self.active = next_active;
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    pub fn draw(&mut self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            let rgba = match self.tiles[y as usize][x as usize] {
                Tile::Empty => [0, 0, 0, 0],
                Tile::Wall => [32, 32, 32, 255],
                Tile::Bug(bug, age) => {
                    let mut rgba = COLORS[bug as usize];
                    rgba[0] = ((rgba[0] as u16) + (age as u16) * 4).min(255) as u8;
                    rgba[1] = ((rgba[1] as u16) + (age as u16) * 4).min(255) as u8;
                    rgba[2] = ((rgba[2] as u16) + (age as u16) * 4).min(255) as u8;
                    rgba
                }
            };

            pixel.copy_from_slice(&rgba);
        }
    }

    pub fn click(&mut self, x: usize, y: usize) {
        // Move our root to the clicked location
        match self.tiles[y][x] {
            Tile::Bug(clicked_id, _) if clicked_id == 0 => {
                self.tiles[y][x] = Tile::Bug(0, MAX_AGE);
                self.roots[0] = (x, y);
                self.active.push((x, y));
            }
            _ => {}
        };
    }
}
