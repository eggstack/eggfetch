#![allow(
    missing_docs,
    clippy::too_many_lines,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::items_after_statements,
    clippy::manual_let_else
)]

//! Streaming decompression chunk-boundary regressions (issue #24).
//!
//! The private stream-to-`AsyncRead` adapter below decompression must not
//! re-emit already-consumed compressed bytes when the source yields multiple
//! items, and empty source items must not surface as EOF. These tests feed
//! the same compressed bytes as one item (control) and as multiple items
//! (fragmented) through `decompress_stream`, plus high-level HTTP/1.1
//! `Content-Length` vs `Transfer-Encoding: chunked` loopback shapes.

use bytes::Bytes;
use eggfetch_core::compression::{decompress_buffered, decompress_stream, DecompressionLimit};
use eggfetch_core::{BoxBytesStream, Client, Timeout};
use futures_util::StreamExt;
use std::time::Duration;

fn plaintext() -> Vec<u8> {
    // Deterministic, compressible, large enough that gzip/brotli produce
    // multi-dozen-byte outputs (so 1-byte and 3-way splits are meaningful).
    let mut v = Vec::new();
    for i in 0..4000u32 {
        v.push(b'A' + u8::try_from(i % 26).unwrap());
        if i % 97 == 0 {
            v.extend_from_slice(b" hello compressed world");
        }
    }
    v
}

#[cfg(any(feature = "compression-gzip", feature = "compression-deflate"))]
fn gzip_compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[cfg(feature = "compression-deflate")]
fn deflate_compress(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

#[cfg(feature = "compression-brotli")]
fn brotli_compress(data: &[u8]) -> Vec<u8> {
    let params = brotli::enc::BrotliEncoderParams::default();
    let mut output = Vec::new();
    brotli::BrotliCompress(&mut &data[..], &mut output, &params).unwrap();
    output
}

#[cfg(feature = "compression-zstd")]
fn zstd_compress(data: &[u8]) -> Vec<u8> {
    zstd::stream::encode_all(data, 1).unwrap()
}

fn split_items(data: &[u8], sizes: &[usize]) -> Vec<Bytes> {
    let mut out = Vec::new();
    let mut pos = 0;
    for &n in sizes {
        if pos >= data.len() {
            break;
        }
        let end = (pos + n).min(data.len());
        out.push(Bytes::copy_from_slice(&data[pos..end]));
        pos = end;
    }
    if pos < data.len() {
        out.push(Bytes::copy_from_slice(&data[pos..]));
    }
    out
}

fn one_byte_items(data: &[u8]) -> Vec<Bytes> {
    data.iter().map(|b| Bytes::copy_from_slice(&[*b])).collect()
}

fn to_stream(chunks: Vec<Bytes>) -> BoxBytesStream {
    let items: Vec<Result<Bytes, eggfetch_core::Error>> = chunks.into_iter().map(Ok).collect();
    Box::pin(futures_util::stream::iter(items))
}

async fn collect_decoded(stream: BoxBytesStream) -> Result<Vec<u8>, eggfetch_core::Error> {
    let mut out = Vec::new();
    let mut s = stream;
    while let Some(chunk) = s.next().await {
        let bytes = chunk?;
        out.extend_from_slice(&bytes);
    }
    Ok(out)
}

async fn decode_streaming(
    _compressed: &[u8],
    chunks: Vec<Bytes>,
    encoding: &str,
) -> Result<Vec<u8>, eggfetch_core::Error> {
    let input = to_stream(chunks);
    let decoded =
        decompress_stream(input, Some(encoding), true, DecompressionLimit::new()).unwrap();
    collect_decoded(decoded).await
}

// ---- Part A: baseline defect proof (gzip + brotli, 3-way split) ----

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_buffered_control_decodes() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let via_buffered = decompress_buffered(&compressed, "gzip", DecompressionLimit::new()).unwrap();
    assert_eq!(&via_buffered[..], &plain[..], "buffered gzip control");
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_one_item_streaming_control_decodes() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let one_item = decode_streaming(&compressed, vec![Bytes::from(compressed.clone())], "gzip")
        .await
        .expect("one-item gzip streaming must decode");
    assert_eq!(one_item, plain, "one-item gzip control");
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_fragmented_three_chunks_matches_plaintext() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    assert!(compressed.len() > 30, "fixture too small to fragment");
    // Fragmented: three non-empty items must decode to the same plaintext.
    let n = compressed.len();
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    assert!(chunks.len() >= 3);
    let fragmented = decode_streaming(&compressed, chunks, "gzip")
        .await
        .expect("fragmented gzip streaming must decode");
    assert_eq!(fragmented, plain, "fragmented gzip must equal plaintext");
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn brotli_buffered_control_decodes() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    let via_buffered = decompress_buffered(&compressed, "br", DecompressionLimit::new()).unwrap();
    assert_eq!(&via_buffered[..], &plain[..], "buffered brotli control");
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn brotli_one_item_streaming_control_decodes() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    let one_item = decode_streaming(&compressed, vec![Bytes::from(compressed.clone())], "br")
        .await
        .expect("one-item brotli streaming must decode");
    assert_eq!(one_item, plain, "one-item brotli control");
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn brotli_fragmented_three_chunks_matches_plaintext() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    assert!(compressed.len() > 30, "fixture too small to fragment");
    let n = compressed.len();
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    assert!(chunks.len() >= 3);
    let fragmented = decode_streaming(&compressed, chunks, "br")
        .await
        .expect("fragmented brotli streaming must decode");
    assert_eq!(fragmented, plain, "fragmented brotli must equal plaintext");
}

// ---- Part C: codec matrix ----

#[cfg(feature = "compression-deflate")]
#[tokio::test]
async fn deflate_fragmented_matches_plaintext() {
    let plain = plaintext();
    let compressed = deflate_compress(&plain);
    let via_buffered =
        decompress_buffered(&compressed, "deflate", DecompressionLimit::new()).unwrap();
    assert_eq!(&via_buffered[..], &plain[..]);
    let n = compressed.len();
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    let fragmented = decode_streaming(&compressed, chunks, "deflate")
        .await
        .expect("fragmented deflate must decode");
    assert_eq!(fragmented, plain);
}

#[cfg(feature = "compression-zstd")]
#[tokio::test]
async fn zstd_fragmented_matches_plaintext() {
    let plain = plaintext();
    let compressed = zstd_compress(&plain);
    let via_buffered = decompress_buffered(&compressed, "zstd", DecompressionLimit::new()).unwrap();
    assert_eq!(&via_buffered[..], &plain[..]);
    let n = compressed.len();
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    let fragmented = decode_streaming(&compressed, chunks, "zstd")
        .await
        .expect("fragmented zstd must decode");
    assert_eq!(fragmented, plain);
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_empty_source_chunks_do_not_terminate_early() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let n = compressed.len();
    let mut chunks = split_items(&compressed, &[n / 3, n / 3]);
    // Interleave empty items between every non-empty chunk.
    let mut with_empty = Vec::new();
    for c in chunks.drain(..) {
        with_empty.push(Bytes::new());
        with_empty.push(c);
    }
    with_empty.push(Bytes::new());
    let decoded = decode_streaming(&compressed, with_empty, "gzip")
        .await
        .expect("empty chunks must be skipped");
    assert_eq!(decoded, plain);
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn brotli_empty_source_chunks_do_not_terminate_early() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    let n = compressed.len();
    let mut chunks = split_items(&compressed, &[n / 3, n / 3]);
    let mut with_empty = Vec::new();
    for c in chunks.drain(..) {
        with_empty.push(Bytes::new());
        with_empty.push(c);
    }
    with_empty.push(Bytes::new());
    let decoded = decode_streaming(&compressed, with_empty, "br")
        .await
        .expect("empty chunks must be skipped");
    assert_eq!(decoded, plain);
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_one_byte_fragments_reconstruct_exactly() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let chunks = one_byte_items(&compressed);
    assert!(chunks.len() > 10);
    let decoded = decode_streaming(&compressed, chunks, "gzip")
        .await
        .expect("1-byte fragments must decode");
    assert_eq!(decoded, plain);
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn brotli_one_byte_fragments_reconstruct_exactly() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    let chunks = one_byte_items(&compressed);
    assert!(chunks.len() > 10);
    let decoded = decode_streaming(&compressed, chunks, "br")
        .await
        .expect("1-byte fragments must decode");
    assert_eq!(decoded, plain);
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn gzip_awkward_header_split_reconstructs() {
    // Split inside the 10-byte gzip header and around the trailer.
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    assert!(compressed.len() > 12);
    let chunks = split_items(&compressed, &[1, 2, 3]);
    let decoded = decode_streaming(&compressed, chunks, "gzip")
        .await
        .expect("awkward gzip splits must decode");
    assert_eq!(decoded, plain);
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-brotli",
    feature = "compression-deflate",
    feature = "compression-zstd"
))]
#[tokio::test]
async fn malformed_fragmented_input_stays_decompression_error() {
    let bad = b"not actually compressed at all, just text ".repeat(4);
    let chunks = split_items(&bad, &[5, 7]);
    #[cfg(feature = "compression-gzip")]
    {
        let err = decode_streaming(&bad, chunks.clone(), "gzip")
            .await
            .expect_err("malformed gzip must fail");
        assert_eq!(err.kind(), "decompression");
    }
    #[cfg(feature = "compression-brotli")]
    {
        let err = decode_streaming(&bad, chunks.clone(), "br")
            .await
            .expect_err("malformed brotli must fail");
        assert_eq!(err.kind(), "decompression");
    }
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn fragmented_size_and_ratio_limits_stay_enforced() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let n = compressed.len();
    // Size limit trips on fragmented input.
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    let input = to_stream(chunks);
    let limit = DecompressionLimit::try_new(Some(10), None).unwrap();
    let decoded = decompress_stream(input, Some("gzip"), true, limit).unwrap();
    let err = collect_decoded(decoded)
        .await
        .expect_err("size limit must trip on fragments");
    assert_eq!(err.kind(), "decoded_body_too_large");
    // Ratio limit trips on fragmented input.
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    let input = to_stream(chunks);
    let limit = DecompressionLimit::try_new(None, Some(0.01)).unwrap();
    let decoded = decompress_stream(input, Some("gzip"), true, limit).unwrap();
    let err = collect_decoded(decoded)
        .await
        .expect_err("ratio limit must trip on fragments");
    assert_eq!(err.kind(), "decompression_ratio_exceeded");
    // Generous limits pass on fragmented input.
    let chunks = split_items(&compressed, &[n / 3, n / 3]);
    let decoded = {
        let input = to_stream(chunks);
        let limit = DecompressionLimit::try_new(Some(10_000_000), Some(1000.0)).unwrap();
        let s = decompress_stream(input, Some("gzip"), true, limit).unwrap();
        collect_decoded(s).await.expect("generous limits must pass")
    };
    assert_eq!(decoded, plain);
}

#[cfg(any(
    feature = "compression-gzip",
    feature = "compression-brotli",
    feature = "compression-deflate",
    feature = "compression-zstd"
))]
#[tokio::test]
async fn unsupported_encoding_stays_unsupported_on_stream() {
    let input = to_stream(vec![Bytes::from_static(b"hello")]);
    let err = match decompress_stream(input, Some("weird-coding"), true, DecompressionLimit::new())
    {
        Ok(_) => panic!("unsupported coding must fail"),
        Err(e) => e,
    };
    assert_eq!(err.kind(), "unsupported_content_encoding");
}

// ---- Part D: high-level transfer shapes ----

async fn serve_shapes_and_collect(
    compressed: Vec<u8>,
    encoding: &str,
    use_chunked: bool,
    raw_mode: bool,
) -> (Vec<u8>, Option<String>, Option<String>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let enc_owned = encoding.to_owned();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await.unwrap();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            if line.trim().is_empty() {
                break;
            }
        }
        let mut stream = reader.into_inner();
        if use_chunked {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Encoding: {enc_owned}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            // Deliberately small transfer chunks so the entity arrives as
            // multiple DATA frames.
            for piece in compressed.chunks(7) {
                let size_line = format!("{:x}\r\n", piece.len());
                stream.write_all(size_line.as_bytes()).await.unwrap();
                stream.write_all(piece).await.unwrap();
                stream.write_all(b"\r\n").await.unwrap();
                stream.flush().await.unwrap();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            stream.write_all(b"0\r\n\r\n").await.unwrap();
        } else {
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Encoding: {enc_owned}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                compressed.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&compressed).await.unwrap();
        }
        stream.flush().await.unwrap();
    });

    let builder = Client::builder()
        .timeout(Timeout::from_secs(15))
        .automatic_decompression(!raw_mode);
    let client = builder.build();
    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let wire_enc = resp.wire_content_encoding().map(str::to_owned);
    let wire_len = resp.wire_content_length().map(str::to_owned);
    let collected = if raw_mode {
        let mut s = resp.raw_bytes_stream().unwrap();
        let mut out = Vec::new();
        while let Some(chunk) = s.next().await {
            out.extend_from_slice(&chunk.unwrap());
        }
        out
    } else {
        let mut s = resp.bytes_stream().unwrap();
        let mut out = Vec::new();
        while let Some(chunk) = s.next().await {
            out.extend_from_slice(&chunk.unwrap());
        }
        out
    };
    let _ = server.await;
    (collected, wire_enc, wire_len)
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn high_level_gzip_content_length_and_chunked_agree() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let (via_len, _, _) = serve_shapes_and_collect(compressed.clone(), "gzip", false, false).await;
    assert_eq!(via_len, plain, "Content-Length gzip must decode");
    let (via_chunked, _, _) =
        serve_shapes_and_collect(compressed.clone(), "gzip", true, false).await;
    assert_eq!(via_chunked, plain, "chunked gzip must decode identically");
}

#[cfg(feature = "compression-brotli")]
#[tokio::test]
async fn high_level_brotli_content_length_and_chunked_agree() {
    let plain = plaintext();
    let compressed = brotli_compress(&plain);
    let (via_len, _, _) = serve_shapes_and_collect(compressed.clone(), "br", false, false).await;
    assert_eq!(via_len, plain, "Content-Length brotli must decode");
    let (via_chunked, _, _) = serve_shapes_and_collect(compressed.clone(), "br", true, false).await;
    assert_eq!(via_chunked, plain, "chunked brotli must decode identically");
}

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn high_level_chunked_raw_mode_returns_exact_bytes() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let (raw, _, _) = serve_shapes_and_collect(compressed.clone(), "gzip", true, true).await;
    assert_eq!(raw, compressed, "raw chunked bytes must be exact");
}

// ---- Part E: metadata / error compat ----

#[cfg(feature = "compression-gzip")]
#[tokio::test]
async fn decoded_response_strips_headers_but_keeps_wire_metadata() {
    let plain = plaintext();
    let compressed = gzip_compress(&plain);
    let (decoded, wire_enc, wire_len) =
        serve_shapes_and_collect(compressed.clone(), "gzip", false, false).await;
    assert_eq!(decoded, plain);
    assert_eq!(wire_enc.as_deref(), Some("gzip"));
    let expected_len = compressed.len().to_string();
    assert_eq!(wire_len.as_deref(), Some(expected_len.as_str()));
    // Visible headers are asserted via a second fetch that inspects headers.
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        loop {
            let mut l = String::new();
            reader.read_line(&mut l).await.unwrap();
            if l.trim().is_empty() {
                break;
            }
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            compressed.len()
        );
        let mut stream = reader.into_inner();
        stream.write_all(header.as_bytes()).await.unwrap();
        stream.write_all(&compressed).await.unwrap();
        stream.flush().await.unwrap();
    });
    let client = Client::builder().timeout(Timeout::from_secs(15)).build();
    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.headers().get("content-encoding").is_none());
    assert!(resp.headers().get("content-length").is_none());
    let body = resp.bytes().await.unwrap();
    assert_eq!(&body[..], &plain[..]);
    let _ = server.await;
}
