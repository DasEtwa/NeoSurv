use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use crate::world::voxel::{
    chunk::{ChunkCoord, ChunkData},
    generation::TerrainGenerator,
    meshing::{ChunkMesh, ChunkNeighborSolidity, build_chunk_mesh_with_neighbors},
};

#[derive(Debug)]
enum PipelineJob {
    Generate {
        coord: ChunkCoord,
        neighbors: ChunkNeighborSolidity,
    },
    Remesh {
        coord: ChunkCoord,
        chunk: ChunkData,
        neighbors: ChunkNeighborSolidity,
    },
    Shutdown,
}

#[derive(Debug)]
pub(crate) struct ChunkBuildResult {
    pub(crate) coord: ChunkCoord,
    pub(crate) chunk: ChunkData,
    pub(crate) mesh: ChunkMesh,
}

#[derive(Debug)]
pub(crate) struct ChunkGenerationPipeline {
    job_tx: Option<mpsc::Sender<PipelineJob>>,
    result_rx: mpsc::Receiver<ChunkBuildResult>,
    workers: Vec<thread::JoinHandle<()>>,
    cancelled: Arc<AtomicBool>,
}

impl ChunkGenerationPipeline {
    pub(crate) fn new(seed: u32, worker_count: usize) -> Self {
        let worker_count = worker_count.max(1);

        let (job_tx, job_rx) = mpsc::channel::<PipelineJob>();
        let (result_tx, result_rx) = mpsc::channel::<ChunkBuildResult>();

        let cancelled = Arc::new(AtomicBool::new(false));
        let job_rx = Arc::new(Mutex::new(job_rx));
        let mut workers = Vec::with_capacity(worker_count);

        for worker_index in 0..worker_count {
            let worker_jobs = Arc::clone(&job_rx);
            let worker_results = result_tx.clone();
            let worker_cancelled = Arc::clone(&cancelled);

            let thread_name = format!("chunk-worker-{worker_index}");
            let handle = thread::Builder::new()
                .name(thread_name)
                .spawn(move || {
                    let generator = TerrainGenerator::new(seed);

                    loop {
                        if worker_cancelled.load(Ordering::Acquire) {
                            break;
                        }

                        let job = {
                            let receiver = worker_jobs
                                .lock()
                                .expect("chunk job receiver mutex poisoned");
                            receiver.recv()
                        };

                        match job {
                            Ok(PipelineJob::Generate { coord, neighbors }) => {
                                if worker_cancelled.load(Ordering::Acquire) {
                                    break;
                                }

                                let chunk = generator.generate_chunk(coord);
                                let mesh = build_chunk_mesh_with_neighbors(&chunk, &neighbors);

                                if worker_cancelled.load(Ordering::Acquire) {
                                    break;
                                }

                                if worker_results
                                    .send(ChunkBuildResult { coord, chunk, mesh })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(PipelineJob::Remesh {
                                coord,
                                chunk,
                                neighbors,
                            }) => {
                                if worker_cancelled.load(Ordering::Acquire) {
                                    break;
                                }

                                let mesh = build_chunk_mesh_with_neighbors(&chunk, &neighbors);

                                if worker_cancelled.load(Ordering::Acquire) {
                                    break;
                                }

                                if worker_results
                                    .send(ChunkBuildResult { coord, chunk, mesh })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Ok(PipelineJob::Shutdown) | Err(_) => break,
                        }
                    }
                })
                .expect("failed to spawn chunk worker thread");

            workers.push(handle);
        }

        Self {
            job_tx: Some(job_tx),
            result_rx,
            workers,
            cancelled,
        }
    }

    pub(crate) fn request_generate_chunk(
        &self,
        coord: ChunkCoord,
        neighbors: ChunkNeighborSolidity,
    ) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return false;
        }

        let Some(job_tx) = self.job_tx.as_ref() else {
            return false;
        };

        job_tx
            .send(PipelineJob::Generate { coord, neighbors })
            .is_ok()
    }

    pub(crate) fn request_remesh(
        &self,
        coord: ChunkCoord,
        chunk: ChunkData,
        neighbors: ChunkNeighborSolidity,
    ) -> bool {
        if self.cancelled.load(Ordering::Acquire) {
            return false;
        }

        let Some(job_tx) = self.job_tx.as_ref() else {
            return false;
        };

        job_tx
            .send(PipelineJob::Remesh {
                coord,
                chunk,
                neighbors,
            })
            .is_ok()
    }

    pub(crate) fn drain_completed(&self, max_chunks: usize) -> Vec<ChunkBuildResult> {
        let mut completed = Vec::with_capacity(max_chunks.max(1));

        for _ in 0..max_chunks.max(1) {
            match self.result_rx.try_recv() {
                Ok(chunk) => completed.push(chunk),
                Err(mpsc::TryRecvError::Empty | mpsc::TryRecvError::Disconnected) => break,
            }
        }

        completed
    }
}

impl Drop for ChunkGenerationPipeline {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);

        if let Some(job_tx) = self.job_tx.take() {
            for _ in 0..self.workers.len() {
                let _ = job_tx.send(PipelineJob::Shutdown);
            }
            drop(job_tx);
        }

        while let Some(worker) = self.workers.pop() {
            let _ = worker.join();
        }
    }
}
