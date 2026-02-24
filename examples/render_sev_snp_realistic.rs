/// Renders the realistic SEV-SNP+TPM example to an HTML file for visual inspection.
use std::fs;

fn main() {
    let input = include_str!("../tests/sev_snp_realistic.obgraph");
    let svg = mdbook_obgraph::process(input).expect("process failed");
    let html = format!(
        r#"<!DOCTYPE html>
<html><head><meta charset="utf-8"><title>SEV-SNP + TPM (Realistic)</title>
<style>body {{ margin: 20px; background: #f0f0f0; }}</style>
</head><body>
<h1>SEV-SNP + TPM Attestation — Realistic Field Names</h1>
{svg}
</body></html>"#
    );
    fs::write("examples/sev_snp_realistic_output.html", &html).expect("write failed");
    println!(
        "Wrote examples/sev_snp_realistic_output.html ({} bytes)",
        html.len()
    );
}
