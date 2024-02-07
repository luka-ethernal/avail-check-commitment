use std::sync::Arc;

use avail_subxt::api::runtime_types::avail_core::header::extension::{v1, v2, HeaderExtension};
use avail_subxt::subxt_rpc::types::BlockNumber;
use avail_subxt::{config::substrate::H256, subxt_rpc::RpcParams};
use kate_recovery::data::Cell;
use kate_recovery::matrix::{Dimensions, Position};
use kate_recovery::{commitments, proof};

const CELL_SIZE: usize = 32;
const PROOF_SIZE: usize = 48;
pub const CELL_WITH_PROOF_SIZE: usize = CELL_SIZE + PROOF_SIZE;

fn extract_kate(extension: &HeaderExtension) -> (u16, u16, H256, Vec<u8>) {
    match &extension {
        HeaderExtension::V1(v1::HeaderExtension {
            commitment: kate, ..
        }) => (
            kate.rows,
            kate.cols,
            kate.data_root,
            kate.commitment.clone(),
        ),
        HeaderExtension::V2(v2::HeaderExtension {
            commitment: kate, ..
        }) => (
            kate.rows,
            kate.cols,
            kate.data_root,
            kate.commitment.clone(),
        ),
    }
}

#[tokio::main]
async fn main() {
    let (client, _) = avail_subxt::build_client("wss://san1ty.avail.tools:443/ws", false)
        .await
        .unwrap();
    let head_hash = client
        .rpc()
        .block_hash(Some(BlockNumber::from(382424u64)))
        .await
        .unwrap()
        .unwrap();
    let header = client.rpc().header(Some(head_hash)).await.unwrap().unwrap();

    let pp = Arc::new(kate_recovery::couscous::public_params());
    let positions = vec![Position::new(20, 192), Position::new(128, 136)];

    let mut params = RpcParams::new();
    params.push(positions.clone()).unwrap();
    params.push(head_hash).unwrap();

    let proofs: Vec<u8> = client
        .rpc()
        .request("kate_queryProof", params)
        .await
        .unwrap();

    let i = proofs
        .chunks_exact(CELL_WITH_PROOF_SIZE)
        .map(|chunk| chunk.try_into().expect("chunks of 80 bytes size"));

    let proof = positions
        .iter()
        .zip(i)
        .map(|(&position, &content)| Cell { position, content })
        .collect::<Vec<_>>();

    let (rows, cols, _, commitment) = extract_kate(&header.extension);

    let commitments = commitments::from_slice(&commitment).unwrap();
    for p in proof {
        let commitment = commitments[p.position.row as usize];

        let Some(dimensions) = Dimensions::new(rows, cols) else {
            println!("Skipping block with invalid dimensions {rows}x{cols}",);
            return ();
        };

        if dimensions.cols().get() <= 2 {
            println!("more than 2 columns is required");
            return ();
        }

        // let cell = cell.clone();

        let result = proof::verify(&pp, dimensions, &commitment, &p);
        match result {
            Ok(is_verified) => {
                if !is_verified {
                    println!("Unable to verify cell {:?}", p);
                } else {
                    println!("Success verify cell.pos={}", p.position);
                }
            }
            Err(ref e) => println!("Error verifying cell: {e:#}"),
        };
    }
}
