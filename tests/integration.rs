// Integration tests: full pipeline from obgraph text to SVG output.

const PKI_EXAMPLE: &str = r#"
domain "PKI" {
  node ca "Certificate Authority" @root @selected {
    subject.common_name    @trust(always)
    subject.org            @trust(always)
    public_key             @trust(always)
  }

  node cert "Certificate" {
    issuer.common_name
    issuer.org
    subject.common_name    @trust(constrained)
    subject.org            @trust(constrained)
    public_key
    signature
  }
}

domain "Transport" {
  node tls "TLS Session" {
    server_cert
    cipher_suite           @trust(constrained)
  }
}

node revocation "Revocation List" @root {
  crl                      @trust(always)
}

cert <- ca : sign
tls <- cert

cert::issuer.common_name <= ca::subject.common_name
cert::issuer.org <= ca::subject.org
cert::signature <= ca::public_key : verified_by
cert::subject.common_name <= revocation::crl : not_in
"#;

#[test]
fn pki_example_full_pipeline() {
    let result = mdbook_obgraph::process(PKI_EXAMPLE);
    assert!(result.is_ok(), "Full pipeline should succeed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("obgraph-container"), "Output should contain SVG container");
    assert!(svg.contains("Certificate Authority"), "Output should contain CA label");
    assert!(svg.contains("Revocation List"), "Output should contain revocation label");
    assert!(svg.contains("obgraph-link"), "Output should contain link edges");
}

#[test]
fn simple_two_node_pipeline() {
    let input = r#"
node root "Root" @root {
  value @trust(always)
}

node child "Child" {
  check
}

child <- root

child::check <= root::value
"#;
    let result = mdbook_obgraph::process(input);
    assert!(result.is_ok(), "Simple pipeline should succeed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("Root"), "Output should contain Root label");
    assert!(svg.contains("Child"), "Output should contain Child label");
}

#[test]
fn single_root_node() {
    let input = r#"
node solo "Solo Node" @root {
  prop_a @trust(always)
  prop_b @trust(always)
}
"#;
    let result = mdbook_obgraph::process(input);
    assert!(result.is_ok(), "Single root node should succeed: {:?}", result.err());
}

const SEV_SNP_TPM: &str = r#"
domain "Verifier" {
  node System "System Clock" @root {
    current_time             @trust(always)
  }

  node Challenge "Attestation Challenge" @root @selected {
    nonce                    @trust(always)
  }
}

domain "AMD SEV-SNP" {
  node ARK "AMD Root Key" @root {
    subject                  @trust(always)
    issuer
    public_key               @trust(always)
    not_before
    not_after
  }

  node ASK "AMD Signing Key" {
    subject                  @trust(constrained)
    issuer
    public_key
    signature
    not_before
    not_after
  }

  node VCEK "VCEK" {
    subject                  @trust(constrained)
    issuer
    public_key
    signature
    not_before
    not_after
    chip_id
  }

  node Report "Attestation Report" @selected {
    chip_id
    report_data
    tcb_version
    signature
  }
}

domain "AMD KDS" {
  node KDS "Key Distribution Service" @root {
    supported_tcbs           @trust(always)
  }
}

domain "NVD" {
  node NVD "National Vulnerability Database" @root {
    cve_list                 @trust(always)
  }
}

domain "TPM" {
  node MfgCA "Manufacturer CA" @root {
    subject                  @trust(always)
    issuer
    public_key               @trust(always)
    not_before
    not_after
  }

  node EK "Endorsement Key" {
    subject                  @trust(constrained)
    issuer
    public_key
    signature
    not_before
    not_after
  }

  node AK "Attestation Key" {
    public_key
  }

  node Quote "TPM Quote" {
    nonce
    pcr_digest
    measurement              @trust(constrained)
    signature
  }

  node TCGLog "TCG Event Log" {
    event_entries
  }
}

domain "Guest vTPM" {
  node GuestData "Guest Report Data" {
    nonce
    public_key
  }

  node vEK "vTPM EK" {
    subject                  @trust(constrained)
    issuer                   @trust(constrained)
    public_key
    signature
  }

  node vAK "vTPM AK" {
    public_key
  }

  node vQuote "vTPM Quote" {
    nonce
    pcr_digest
    measurement              @trust(constrained)
    signature
  }

  node vTCGLog "vTPM Event Log" {
    event_entries
  }
}

# Links
ASK <- ARK : sign
VCEK <- ASK : sign
Report <- VCEK : sign
EK <- MfgCA : sign
AK <- EK : make_credential
Quote <- AK : sign
TCGLog <- Quote : replay_validate
GuestData <- Report : hash
vEK <- GuestData : sign
vAK <- vEK : make_credential
vQuote <- vAK : sign
vTCGLog <- vQuote : replay_validate

# AMD SEV-SNP constraints
ARK::issuer <= ARK::subject : self_signed
ARK::not_before <= System::current_time : valid_after
ARK::not_after <= System::current_time : valid_before
ASK::issuer <= ARK::subject
ASK::signature <= ARK::public_key : verified_by
ASK::not_before <= System::current_time : valid_after
ASK::not_after <= System::current_time : valid_before
VCEK::issuer <= ASK::subject
VCEK::signature <= ASK::public_key : verified_by
VCEK::not_before <= System::current_time : valid_after
VCEK::not_after <= System::current_time : valid_before
Report::signature <= VCEK::public_key : verified_by
Report::chip_id <= VCEK::chip_id

# TPM constraints
MfgCA::issuer <= MfgCA::subject : self_signed
MfgCA::not_before <= System::current_time : valid_after
MfgCA::not_after <= System::current_time : valid_before
EK::issuer <= MfgCA::subject
EK::signature <= MfgCA::public_key : verified_by
EK::not_before <= System::current_time : valid_after
EK::not_after <= System::current_time : valid_before
AK::public_key <= EK::public_key : make_credential
Quote::signature <= AK::public_key : verified_by
TCGLog::event_entries <= Quote::pcr_digest : replay_validates

# Guest vTPM constraints
vEK::signature <= GuestData::public_key : verified_by
vAK::public_key <= vEK::public_key : make_credential
vQuote::signature <= vAK::public_key : verified_by
vTCGLog::event_entries <= vQuote::pcr_digest : replay_validates

# Cross-domain constraints
Report::chip_id <= TCGLog::event_entries : contains
GuestData::nonce <= Challenge::nonce
Quote::nonce <= Challenge::nonce
Report::tcb_version <= filter(KDS::supported_tcbs, NVD::cve_list) : in
"#;

#[test]
fn sev_snp_tpm_full_pipeline() {
    let result = mdbook_obgraph::process(SEV_SNP_TPM);
    assert!(result.is_ok(), "SEV-SNP+TPM pipeline should succeed: {:?}", result.err());
    let svg = result.unwrap();
    assert!(svg.contains("obgraph-container"), "Output should contain SVG container");
    assert!(svg.contains("AMD Root Key"), "Output should contain ARK label");
    assert!(svg.contains("Attestation Report"), "Output should contain Report label");
    assert!(svg.contains("TPM Quote"), "Output should contain Quote label");
    assert!(svg.contains("vTPM Event Log"), "Output should contain vTCGLog label");
}
