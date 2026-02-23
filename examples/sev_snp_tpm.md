# SEV-SNP and TPM Attestation

This file models the attestation chain for AMD SEV-SNP and TPM using the
obgraph syntax.

```obgraph
# ==========================================================================
# Verifier domain — axiomatic trust sources
# ==========================================================================

domain "Verifier" {
  node System "System Clock" @anchored {
    current_time             @constrained
  }

  node Challenge "Attestation Challenge" @anchored @selected {
    nonce                    @constrained
  }
}

# ==========================================================================
# AMD SEV-SNP domain — hardware root of trust chain
# ==========================================================================

domain "AMD SEV-SNP" {
  # AMD Root Key — self-signed trust root
  node ARK "AMD Root Key" @anchored {
    subject                  @constrained
    issuer
    public_key               @constrained
    not_before
    not_after
  }

  # AMD Signing Key — signed by ARK
  node ASK "AMD Signing Key" {
    subject                  @critical
    issuer
    public_key
    signature
    not_before
    not_after
  }

  # Versioned Chip Endorsement Key — signed by ASK
  node VCEK "VCEK" {
    subject                  @critical
    issuer
    public_key
    signature
    not_before
    not_after
    chip_id
  }

  # SEV-SNP Attestation Report — signed by VCEK
  node Report "Attestation Report" @selected {
    chip_id
    report_data
    tcb_version
    signature
  }
}

# ==========================================================================
# AMD KDS domain — key distribution
# ==========================================================================

domain "AMD KDS" {
  node KDS "Key Distribution Service" @anchored {
    supported_tcbs           @constrained
  }
}

# ==========================================================================
# NVD domain — vulnerability database
# ==========================================================================

domain "NVD" {
  node NVD "National Vulnerability Database" @anchored {
    cve_list                 @constrained
  }
}

# ==========================================================================
# TPM domain — physical TPM trust chain
# ==========================================================================

domain "TPM" {
  # TPM Manufacturer CA — self-signed trust root
  node MfgCA "Manufacturer CA" @anchored {
    subject                  @constrained
    issuer
    public_key               @constrained
    not_before
    not_after
  }

  # Endorsement Key — signed by MfgCA
  node EK "Endorsement Key" {
    subject                  @critical
    issuer
    public_key
    signature
    not_before
    not_after
  }

  # Attestation Key — credentialed via EK
  node AK "Attestation Key" {
    public_key
  }

  # TPM Quote — signed by AK
  node Quote "TPM Quote" {
    nonce
    pcr_digest
    measurement              @critical
    signature
  }

  # TCG Event Log — validated against quote
  node TCGLog "TCG Event Log" {
    event_entries
  }
}

# ==========================================================================
# Guest vTPM domain — virtual TPM in the guest
# ==========================================================================

domain "Guest vTPM" {
  # Guest report data bridges SNP and vTPM
  node GuestData "Guest Report Data" {
    nonce
    public_key
  }

  # vTPM Endorsement Key — signed by GuestData.public_key
  node vEK "vTPM EK" {
    subject                  @critical
    issuer                   @critical
    public_key
    signature
  }

  # vTPM Attestation Key — credentialed via vEK
  node vAK "vTPM AK" {
    public_key
  }

  # vTPM Quote — signed by vAK
  node vQuote "vTPM Quote" {
    nonce
    pcr_digest
    measurement              @critical
    signature
  }

  # vTPM Event Log — validated against vQuote
  node vTCGLog "vTPM Event Log" {
    event_entries
  }
}

# ==========================================================================
# Links (trust flows right to left)
# ==========================================================================

# AMD SEV-SNP chain
ASK <- ARK : sign
VCEK <- ASK : sign
Report <- VCEK : sign

# TPM chain
EK <- MfgCA : sign
AK <- EK : make_credential
Quote <- AK : sign
TCGLog <- Quote : replay_validate

# Guest vTPM chain
GuestData <- Report : hash
vEK <- GuestData : sign
vAK <- vEK : make_credential
vQuote <- vAK : sign
vTCGLog <- vQuote : replay_validate

# ==========================================================================
# Intra-domain constraints: AMD SEV-SNP
# ==========================================================================

# ARK is self-signed
ARK::issuer <= ARK::subject : self_signed

# ARK time validity
ARK::not_before <= System::current_time : valid_after
ARK::not_after <= System::current_time : valid_before

# ASK verified by ARK
ASK::issuer <= ARK::subject
ASK::signature <= ARK::public_key : verified_by
ASK::not_before <= System::current_time : valid_after
ASK::not_after <= System::current_time : valid_before

# VCEK verified by ASK
VCEK::issuer <= ASK::subject
VCEK::signature <= ASK::public_key : verified_by
VCEK::not_before <= System::current_time : valid_after
VCEK::not_after <= System::current_time : valid_before

# Report verified by VCEK
Report::signature <= VCEK::public_key : verified_by
Report::chip_id <= VCEK::chip_id

# ==========================================================================
# Intra-domain constraints: TPM
# ==========================================================================

# MfgCA is self-signed
MfgCA::issuer <= MfgCA::subject : self_signed

# MfgCA time validity
MfgCA::not_before <= System::current_time : valid_after
MfgCA::not_after <= System::current_time : valid_before

# EK verified by MfgCA
EK::issuer <= MfgCA::subject
EK::signature <= MfgCA::public_key : verified_by
EK::not_before <= System::current_time : valid_after
EK::not_after <= System::current_time : valid_before

# AK credentialed via EK
AK::public_key <= EK::public_key : make_credential

# Quote verified by AK
Quote::signature <= AK::public_key : verified_by

# TCGLog validated against quote
TCGLog::event_entries <= Quote::pcr_digest : replay_validates

# ==========================================================================
# Intra-domain constraints: Guest vTPM
# ==========================================================================

# vEK signed by GuestData
vEK::signature <= GuestData::public_key : verified_by

# vAK credentialed via vEK
vAK::public_key <= vEK::public_key : make_credential

# vQuote verified by vAK
vQuote::signature <= vAK::public_key : verified_by

# vTCGLog validated against vQuote
vTCGLog::event_entries <= vQuote::pcr_digest : replay_validates

# ==========================================================================
# Cross-domain constraints
# ==========================================================================

# SNP report chip_id must be in TPM event log
Report::chip_id <= TCGLog::event_entries : contains

# Guest report data is a hash of the SNP report
GuestData::nonce <= Challenge::nonce

# TPM quote nonce matches the attestation challenge
Quote::nonce <= Challenge::nonce

# TCB version policy: filter AMD supported TCBs by NVD vulnerabilities,
# then constrain the report's TCB version to the safe set.
Report::tcb_version <= filter(KDS::supported_tcbs, NVD::cve_list) : in
```
