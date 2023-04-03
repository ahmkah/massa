// Copyright (c) 2022 MASSA LABS <info@massa.net>
/// An overview of what is a Denunciation and what it is used for can be found here
/// https://github.com/massalabs/massa/discussions/3113
use std::ops::Bound::{Excluded, Included};

use nom::{
    error::{context, ContextError, ParseError},
    sequence::tuple,
    IResult, Parser,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use thiserror::Error;

use crate::block_header::{
    BlockHeader, BlockHeaderDenunciationData, BlockHeaderSerializer, SecuredHeader,
};
use crate::endorsement::{
    Endorsement, EndorsementDenunciationData, EndorsementSerializer, SecureShareEndorsement,
};
use crate::slot::{Slot, SlotDeserializer, SlotSerializer};

use massa_hash::{Hash, HashDeserializer, HashSerializer};
use massa_serialization::{
    Deserializer, SerializeError, Serializer, U32VarIntDeserializer, U32VarIntSerializer,
};
use massa_signature::{
    MassaSignatureError, PublicKey, PublicKeyDeserializer, Signature, SignatureDeserializer,
};

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
struct EndorsementDenunciation {
    public_key: PublicKey,
    slot: Slot,
    index: u32,
    hash_1: Hash,
    hash_2: Hash,
    signature_1: Signature,
    signature_2: Signature,
}

impl EndorsementDenunciation {
    /// Compute hash for Endorsement content
    fn compute_content_hash(content: &Endorsement) -> Result<Hash, SerializeError> {
        let mut buf = Vec::new();
        let endorsement_serializer = EndorsementSerializer::new();
        endorsement_serializer.serialize(content, &mut buf)?;
        Ok(Hash::compute_from(&buf))
    }

    /// Rebuild full hash of SecureShareEndorsement from given arguments
    fn compute_hash_for_sig_verif(
        public_key: &PublicKey,
        slot: &Slot,
        index: &u32,
        content_hash: &Hash,
    ) -> Result<Hash, SerializeError> {
        let mut hash_data = Vec::new();

        // Public key
        hash_data.extend(public_key.to_bytes());
        // Ser slot & index
        let denunciation_data = EndorsementDenunciationData::new(*slot, *index);
        hash_data.extend(&denunciation_data.to_bytes());
        // Add content hash
        hash_data.extend(content_hash.to_bytes());

        Ok(Hash::compute_from(&hash_data))
    }
}

#[allow(dead_code)]
#[derive(Debug, PartialEq)]
struct BlockHeaderDenunciation {
    public_key: PublicKey,
    slot: Slot,
    hash_1: Hash,
    hash_2: Hash,
    signature_1: Signature,
    signature_2: Signature,
}

impl BlockHeaderDenunciation {
    /// Compute hash for Block header content
    fn compute_content_hash(content: &BlockHeader) -> Result<Hash, SerializeError> {
        let mut buf = Vec::new();
        let block_header_serializer = BlockHeaderSerializer::new();
        block_header_serializer.serialize(content, &mut buf)?;
        Ok(Hash::compute_from(&buf))
    }

    /// Rebuild full hash of SecuredHeader from given arguments
    fn compute_hash_for_sig_verif(
        public_key: &PublicKey,
        slot: &Slot,
        content_hash: &Hash,
    ) -> Result<Hash, SerializeError> {
        let mut hash_data = Vec::new();
        // let mut buf = Vec::new();
        // let de_data_serializer = DenunciationDataSerializer::new();

        // Public key
        hash_data.extend(public_key.to_bytes());

        // Ser slot
        // let denunciation_data = DenunciationData::BlockHeader(*slot);
        // de_data_serializer.serialize(&denunciation_data, &mut buf)?;
        let de_data = BlockHeaderDenunciationData::new(*slot);
        hash_data.extend(de_data.to_bytes());
        // hash_data.extend(&buf);
        // buf.clear();

        // Add content hash
        hash_data.extend(content_hash.to_bytes());

        Ok(Hash::compute_from(&hash_data))
    }
}

#[derive(Debug, PartialEq)]
enum Denunciation {
    Endorsement(EndorsementDenunciation),
    BlockHeader(BlockHeaderDenunciation),
}

#[allow(dead_code)]
impl Denunciation {
    /// Check if it is a Denunciation of several endorsements
    fn is_for_endorsement(&self) -> bool {
        matches!(self, Denunciation::Endorsement(_))
    }

    /// Check if it is a Denunciation of several block headers
    fn is_for_block_header(&self) -> bool {
        matches!(self, Denunciation::BlockHeader(_))
    }

    /// Check if it is a Denunciation for this endorsement
    #[cfg(test)]
    fn is_also_for_endorsement(
        &self,
        s_endorsement: &SecureShareEndorsement,
    ) -> Result<bool, DenunciationError> {
        match self {
            Denunciation::BlockHeader(_) => Ok(false),
            Denunciation::Endorsement(endo_de) => {
                let content_hash =
                    EndorsementDenunciation::compute_content_hash(&s_endorsement.content)?;

                let hash_ = EndorsementDenunciation::compute_hash_for_sig_verif(
                    &endo_de.public_key,
                    &endo_de.slot,
                    &endo_de.index,
                    &content_hash,
                );

                if let Ok(hash) = hash_ {
                    Ok(endo_de.slot == s_endorsement.content.slot
                        && endo_de.index == s_endorsement.content.index
                        && endo_de.public_key == s_endorsement.content_creator_pub_key
                        && endo_de.hash_1 != content_hash
                        && endo_de.hash_2 != content_hash
                        && endo_de
                            .public_key
                            .verify_signature(&hash, &s_endorsement.signature)
                            .is_ok())
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Check if it is a Denunciation for this block header
    #[cfg(test)]
    fn is_also_for_block_header(
        &self,
        s_block_header: &SecuredHeader,
    ) -> Result<bool, DenunciationError> {
        match self {
            Denunciation::Endorsement(_) => Ok(false),
            Denunciation::BlockHeader(endo_bh) => {
                let content_hash =
                    BlockHeaderDenunciation::compute_content_hash(&s_block_header.content)?;

                let hash_ = BlockHeaderDenunciation::compute_hash_for_sig_verif(
                    &endo_bh.public_key,
                    &endo_bh.slot,
                    &content_hash,
                );

                if let Ok(hash) = hash_ {
                    Ok(endo_bh.slot == s_block_header.content.slot
                        && endo_bh.public_key == s_block_header.content_creator_pub_key
                        && endo_bh.hash_1 != content_hash
                        && endo_bh.hash_2 != content_hash
                        && endo_bh
                            .public_key
                            .verify_signature(&hash, &s_block_header.signature)
                            .is_ok())
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// Check if Denunciation is valid
    /// Should be used if received from the network (prevent against invalid or attacker crafted denunciation)
    fn is_valid(&self) -> Result<bool, DenunciationError> {
        let (signature_1, signature_2, hash_1, hash_2, public_key) = match self {
            Denunciation::Endorsement(de) => {
                let hash_1 = EndorsementDenunciation::compute_hash_for_sig_verif(
                    &de.public_key,
                    &de.slot,
                    &de.index,
                    &de.hash_1,
                )?;
                let hash_2 = EndorsementDenunciation::compute_hash_for_sig_verif(
                    &de.public_key,
                    &de.slot,
                    &de.index,
                    &de.hash_2,
                )?;

                (
                    de.signature_1,
                    de.signature_2,
                    hash_1,
                    hash_2,
                    de.public_key,
                )
            }
            Denunciation::BlockHeader(de) => {
                let hash_1 = BlockHeaderDenunciation::compute_hash_for_sig_verif(
                    &de.public_key,
                    &de.slot,
                    &de.hash_1,
                )?;
                let hash_2 = BlockHeaderDenunciation::compute_hash_for_sig_verif(
                    &de.public_key,
                    &de.slot,
                    &de.hash_2,
                )?;

                (
                    de.signature_1,
                    de.signature_2,
                    hash_1,
                    hash_2,
                    de.public_key,
                )
            }
        };

        Ok(hash_1 != hash_2
            && signature_1 != signature_2
            && public_key.verify_signature(&hash_1, &signature_1).is_ok()
            && public_key.verify_signature(&hash_2, &signature_2).is_ok())
    }
}

/// Create a new Denunciation from 2 SecureShareEndorsement
impl TryFrom<(&SecureShareEndorsement, &SecureShareEndorsement)> for Denunciation {
    type Error = DenunciationError;

    fn try_from(
        (s_e1, s_e2): (&SecureShareEndorsement, &SecureShareEndorsement),
    ) -> Result<Self, Self::Error> {
        // Cannot use the same endorsement twice
        if s_e1.id == s_e2.id {
            return Err(DenunciationError::InvalidInput);
        }

        // In order to create a Denunciation, there should be the same
        // slot, index & public key
        if s_e1.content.slot != s_e2.content.slot
            || s_e1.content.index != s_e2.content.index
            || s_e1.content_creator_pub_key != s_e2.content_creator_pub_key
        {
            return Err(DenunciationError::InvalidInput);
        }

        // Check sig of s_e1 with s_e1.public_key, s_e1.slot, s_e1.index
        let s_e1_hash_content = EndorsementDenunciation::compute_content_hash(&s_e1.content)?;
        let s_e1_hash = EndorsementDenunciation::compute_hash_for_sig_verif(
            &s_e1.content_creator_pub_key,
            &s_e1.content.slot,
            &s_e1.content.index,
            &s_e1_hash_content,
        )?;
        // Check sig of s_e2 but with s_e1.public_key, s_e1.slot, s_e1.index
        let s_e2_hash_content = EndorsementDenunciation::compute_content_hash(&s_e2.content)?;
        let s_e2_hash = EndorsementDenunciation::compute_hash_for_sig_verif(
            &s_e1.content_creator_pub_key,
            &s_e1.content.slot,
            &s_e1.content.index,
            &s_e2_hash_content,
        )?;

        s_e1.content_creator_pub_key
            .verify_signature(&s_e1_hash, &s_e1.signature)?;
        s_e1.content_creator_pub_key
            .verify_signature(&s_e2_hash, &s_e2.signature)?;

        Ok(Denunciation::Endorsement(EndorsementDenunciation {
            public_key: s_e1.content_creator_pub_key,
            slot: s_e1.content.slot,
            index: s_e1.content.index,
            signature_1: s_e1.signature,
            signature_2: s_e2.signature,
            hash_1: s_e1_hash_content,
            hash_2: s_e2_hash_content,
        }))
    }
}

/// Create a new Denunciation from 2 SecureHeader
impl TryFrom<(&SecuredHeader, &SecuredHeader)> for Denunciation {
    type Error = DenunciationError;

    fn try_from((s_bh1, s_bh2): (&SecuredHeader, &SecuredHeader)) -> Result<Self, Self::Error> {
        // Cannot use the same block header twice
        // In order to create a Denunciation, there should be the same slot, index & public key
        if s_bh1.content.slot != s_bh2.content.slot
            || s_bh1.content_creator_pub_key != s_bh2.content_creator_pub_key
            || s_bh1.id == s_bh2.id
        {
            return Err(DenunciationError::InvalidInput);
        }

        // Check sig of s_bh2 but with s_bh1.public_key, s_bh1.slot, s_bh1.index
        let s_bh1_hash_content = BlockHeaderDenunciation::compute_content_hash(&s_bh1.content)?;
        let s_bh1_hash = BlockHeaderDenunciation::compute_hash_for_sig_verif(
            &s_bh1.content_creator_pub_key,
            &s_bh1.content.slot,
            &s_bh1_hash_content,
        )?;
        let s_bh2_hash_content = BlockHeaderDenunciation::compute_content_hash(&s_bh2.content)?;
        let s_bh2_hash = BlockHeaderDenunciation::compute_hash_for_sig_verif(
            &s_bh1.content_creator_pub_key,
            &s_bh1.content.slot,
            &s_bh2_hash_content,
        )?;

        s_bh1
            .content_creator_pub_key
            .verify_signature(&s_bh1_hash, &s_bh1.signature)?;
        s_bh1
            .content_creator_pub_key
            .verify_signature(&s_bh2_hash, &s_bh2.signature)?;

        Ok(Denunciation::BlockHeader(BlockHeaderDenunciation {
            public_key: s_bh1.content_creator_pub_key,
            slot: s_bh1.content.slot,
            signature_1: s_bh1.signature,
            signature_2: s_bh2.signature,
            hash_1: s_bh1_hash_content,
            hash_2: s_bh2_hash_content,
        }))
    }
}

#[allow(missing_docs)]
#[derive(IntoPrimitive, Debug, TryFromPrimitive)]
#[repr(u32)]
pub enum DenunciationTypeId {
    Endorsement = 0,
    BlockHeader = 1,
}

impl From<&Denunciation> for DenunciationTypeId {
    fn from(value: &Denunciation) -> Self {
        match value {
            Denunciation::Endorsement(_) => DenunciationTypeId::Endorsement,
            Denunciation::BlockHeader(_) => DenunciationTypeId::BlockHeader,
        }
    }
}

/// Denunciation error
#[allow(missing_docs)]
#[derive(Error, Debug)]
pub enum DenunciationError {
    #[error("Invalid endorsements or block headers, cannot create denunciation")]
    InvalidInput,
    #[error("signature error: {0}")]
    Signature(#[from] MassaSignatureError),
    #[error("serialization error: {0}")]
    Serialization(#[from] SerializeError),
}

// Serialization / Deserialization

/// Serializer for `EndorsementDenunciation`
struct EndorsementDenunciationSerializer {
    slot_serializer: SlotSerializer,
    u32_serializer: U32VarIntSerializer,
    hash_serializer: HashSerializer,
}

impl EndorsementDenunciationSerializer {
    /// Creates a new `EndorsementDenunciationSerializer`
    const fn new() -> Self {
        Self {
            slot_serializer: SlotSerializer::new(),
            u32_serializer: U32VarIntSerializer::new(),
            hash_serializer: HashSerializer::new(),
        }
    }
}

impl Default for EndorsementDenunciationSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer<EndorsementDenunciation> for EndorsementDenunciationSerializer {
    fn serialize(
        &self,
        value: &EndorsementDenunciation,
        buffer: &mut Vec<u8>,
    ) -> Result<(), SerializeError> {
        buffer.extend(value.public_key.to_bytes());
        self.slot_serializer.serialize(&value.slot, buffer)?;
        self.u32_serializer.serialize(&value.index, buffer)?;
        self.hash_serializer.serialize(&value.hash_1, buffer)?;
        self.hash_serializer.serialize(&value.hash_2, buffer)?;
        buffer.extend(value.signature_1.to_bytes());
        buffer.extend(value.signature_2.to_bytes());
        Ok(())
    }
}

/// Deserializer for `EndorsementDenunciation`
struct EndorsementDenunciationDeserializer {
    slot_deserializer: SlotDeserializer,
    index_deserializer: U32VarIntDeserializer,
    hash_deserializer: HashDeserializer,
    pubkey_deserializer: PublicKeyDeserializer,
    signature_deserializer: SignatureDeserializer,
}

impl EndorsementDenunciationDeserializer {
    /// Creates a new `EndorsementDeserializer`
    const fn new(thread_count: u8, endorsement_count: u32) -> Self {
        EndorsementDenunciationDeserializer {
            slot_deserializer: SlotDeserializer::new(
                (Included(0), Included(u64::MAX)),
                (Included(0), Excluded(thread_count)),
            ),
            index_deserializer: U32VarIntDeserializer::new(
                Included(0),
                Excluded(endorsement_count),
            ),
            hash_deserializer: HashDeserializer::new(),
            pubkey_deserializer: PublicKeyDeserializer::new(),
            signature_deserializer: SignatureDeserializer::new(),
        }
    }
}

impl Deserializer<EndorsementDenunciation> for EndorsementDenunciationDeserializer {
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], EndorsementDenunciation, E> {
        context(
            "Failed Endorsement Denunciation deserialization",
            tuple((
                context("Failed public key deserialization", |input| {
                    self.pubkey_deserializer.deserialize(input)
                }),
                context("Failed slot deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed slot deserialization", |input| {
                    self.index_deserializer.deserialize(input)
                }),
                context("Failed hash 1 deserialization", |input| {
                    self.hash_deserializer.deserialize(input)
                }),
                context("Failed hash 2 deserialization", |input| {
                    self.hash_deserializer.deserialize(input)
                }),
                context("Failed signature 1 deserialization", |input| {
                    self.signature_deserializer.deserialize(input)
                }),
                context("Failed signature 2 deserialization", |input| {
                    self.signature_deserializer.deserialize(input)
                }),
            )),
        )
        .map(
            |(public_key, slot, index, hash_1, hash_2, signature_1, signature_2)| {
                EndorsementDenunciation {
                    public_key,
                    slot,
                    index,
                    hash_1,
                    hash_2,
                    signature_1,
                    signature_2,
                }
            },
        )
        .parse(buffer)
    }
}

/// Serializer for `BlockHeaderDenunciation`
struct BlockHeaderDenunciationSerializer {
    slot_serializer: SlotSerializer,
    hash_serializer: HashSerializer,
}

impl BlockHeaderDenunciationSerializer {
    /// Creates a new `BlockHeaderDenunciationSerializer`
    const fn new() -> Self {
        Self {
            slot_serializer: SlotSerializer::new(),
            hash_serializer: HashSerializer::new(),
        }
    }
}

impl Default for BlockHeaderDenunciationSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer<BlockHeaderDenunciation> for BlockHeaderDenunciationSerializer {
    fn serialize(
        &self,
        value: &BlockHeaderDenunciation,
        buffer: &mut Vec<u8>,
    ) -> Result<(), SerializeError> {
        buffer.extend(value.public_key.to_bytes());
        self.slot_serializer.serialize(&value.slot, buffer)?;
        self.hash_serializer.serialize(&value.hash_1, buffer)?;
        self.hash_serializer.serialize(&value.hash_2, buffer)?;
        buffer.extend(value.signature_1.to_bytes());
        buffer.extend(value.signature_2.to_bytes());
        Ok(())
    }
}

/// Deserializer for `BlockHeaderDenunciation`
struct BlockHeaderDenunciationDeserializer {
    slot_deserializer: SlotDeserializer,
    hash_deserializer: HashDeserializer,
    pubkey_deserializer: PublicKeyDeserializer,
    signature_deserializer: SignatureDeserializer,
}

impl BlockHeaderDenunciationDeserializer {
    /// Creates a new `BlockHeaderDenunciationDeserializer`
    pub const fn new(thread_count: u8) -> Self {
        Self {
            slot_deserializer: SlotDeserializer::new(
                (Included(0), Included(u64::MAX)),
                (Included(0), Excluded(thread_count)),
            ),
            hash_deserializer: HashDeserializer::new(),
            pubkey_deserializer: PublicKeyDeserializer::new(),
            signature_deserializer: SignatureDeserializer::new(),
        }
    }
}

impl Deserializer<BlockHeaderDenunciation> for BlockHeaderDenunciationDeserializer {
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], BlockHeaderDenunciation, E> {
        context(
            "Failed BlockHeader Denunciation deserialization",
            tuple((
                context("Failed public key deserialization", |input| {
                    self.pubkey_deserializer.deserialize(input)
                }),
                context("Failed slot deserialization", |input| {
                    self.slot_deserializer.deserialize(input)
                }),
                context("Failed hash 1 deserialization", |input| {
                    self.hash_deserializer.deserialize(input)
                }),
                context("Failed hash 2 deserialization", |input| {
                    self.hash_deserializer.deserialize(input)
                }),
                context("Failed signature 1 deserialization", |input| {
                    self.signature_deserializer.deserialize(input)
                }),
                context("Failed signature 2 deserialization", |input| {
                    self.signature_deserializer.deserialize(input)
                }),
            )),
        )
        .map(
            |(public_key, slot, hash_1, hash_2, signature_1, signature_2)| {
                BlockHeaderDenunciation {
                    public_key,
                    slot,
                    hash_1,
                    hash_2,
                    signature_1,
                    signature_2,
                }
            },
        )
        .parse(buffer)
    }
}

/// Serializer for `Denunciation`
pub struct DenunciationSerializer {
    endo_de_serializer: EndorsementDenunciationSerializer,
    blkh_de_serializer: BlockHeaderDenunciationSerializer,
    type_id_serializer: U32VarIntSerializer,
}

impl DenunciationSerializer {
    /// Creates a new `BlockHeaderDenunciationSerializer`
    pub const fn new() -> Self {
        Self {
            endo_de_serializer: EndorsementDenunciationSerializer::new(),
            blkh_de_serializer: BlockHeaderDenunciationSerializer::new(),
            type_id_serializer: U32VarIntSerializer::new(),
        }
    }
}

impl Default for DenunciationSerializer {
    fn default() -> Self {
        Self::new()
    }
}

impl Serializer<Denunciation> for DenunciationSerializer {
    fn serialize(&self, value: &Denunciation, buffer: &mut Vec<u8>) -> Result<(), SerializeError> {
        let de_type_id = DenunciationTypeId::from(value);
        self.type_id_serializer
            .serialize(&u32::from(de_type_id), buffer)?;
        match value {
            Denunciation::Endorsement(de) => {
                self.endo_de_serializer.serialize(de, buffer)?;
            }
            Denunciation::BlockHeader(de) => {
                self.blkh_de_serializer.serialize(de, buffer)?;
            }
        }
        Ok(())
    }
}

const DENUNCIATION_TYPE_ID_VARIANT_COUNT: u32 =
    std::mem::variant_count::<DenunciationTypeId>() as u32;

/// Deserializer for `Denunciation`
pub struct DenunciationDeserializer {
    endo_de_deserializer: EndorsementDenunciationDeserializer,
    blkh_de_deserializer: BlockHeaderDenunciationDeserializer,
    type_id_deserializer: U32VarIntDeserializer,
}

impl DenunciationDeserializer {
    /// Creates a new `DenunciationDeserializer`
    pub const fn new(thread_count: u8, endorsement_count: u32) -> Self {
        Self {
            endo_de_deserializer: EndorsementDenunciationDeserializer::new(
                thread_count,
                endorsement_count,
            ),
            blkh_de_deserializer: BlockHeaderDenunciationDeserializer::new(thread_count),
            type_id_deserializer: U32VarIntDeserializer::new(
                Included(0),
                Excluded(DENUNCIATION_TYPE_ID_VARIANT_COUNT),
            ),
        }
    }
}

impl Deserializer<Denunciation> for DenunciationDeserializer {
    fn deserialize<'a, E: ParseError<&'a [u8]> + ContextError<&'a [u8]>>(
        &self,
        buffer: &'a [u8],
    ) -> IResult<&'a [u8], Denunciation, E> {
        let (rem, de_type_id_) = context("Failed Denunciation type id deserialization", |input| {
            self.type_id_deserializer.deserialize(input)
        })
        .parse(buffer)?;

        let de_type_id = DenunciationTypeId::try_from(de_type_id_).map_err(|_| {
            nom::Err::Error(ParseError::from_error_kind(
                buffer,
                nom::error::ErrorKind::Fail,
            ))
        })?;

        match de_type_id {
            DenunciationTypeId::Endorsement => {
                let (rem2, endo_de) = self.endo_de_deserializer.deserialize(rem)?;
                IResult::Ok((rem2, Denunciation::Endorsement(endo_de)))
            }
            DenunciationTypeId::BlockHeader => {
                let (rem2, blkh_de) = self.blkh_de_deserializer.deserialize(rem)?;
                IResult::Ok((rem2, Denunciation::BlockHeader(blkh_de)))
            }
        }
    }
}

// End Ser / Der

#[cfg(test)]
mod tests {
    use super::*;
    use massa_serialization::DeserializeError;

    use crate::block_id::BlockId;
    use crate::endorsement::{
        Endorsement, EndorsementSerializer, EndorsementSerializerLW, SecureShareEndorsement,
    };

    use crate::config::{ENDORSEMENT_COUNT, THREAD_COUNT};
    use crate::secure_share::{Id, SecureShareContent};
    use massa_signature::KeyPair;

    /// Helper for Endorsement denunciation
    fn gen_endorsements_for_denunciation() -> (
        Slot,
        KeyPair,
        SecureShareEndorsement,
        SecureShareEndorsement,
        SecureShareEndorsement,
    ) {
        let keypair = KeyPair::generate();

        let slot = Slot::new(3, 7);
        let endorsement_1 = Endorsement {
            slot,
            index: 0,
            endorsed_block: BlockId(Hash::compute_from("blk1".as_bytes())),
        };

        let v_endorsement1 =
            Endorsement::new_verifiable(endorsement_1, EndorsementSerializer::new(), &keypair)
                .unwrap();

        let endorsement_2 = Endorsement {
            slot,
            index: 0,
            endorsed_block: BlockId(Hash::compute_from("blk2".as_bytes())),
        };

        let v_endorsement2 =
            Endorsement::new_verifiable(endorsement_2, EndorsementSerializer::new(), &keypair)
                .unwrap();

        let endorsement_3 = Endorsement {
            slot,
            index: 0,
            endorsed_block: BlockId(Hash::compute_from("blk3".as_bytes())),
        };
        let v_endorsement_3 =
            Endorsement::new_verifiable(endorsement_3, EndorsementSerializer::new(), &keypair)
                .unwrap();

        return (
            slot,
            keypair,
            v_endorsement1,
            v_endorsement2,
            v_endorsement_3,
        );
    }

    #[test]
    fn test_endorsement_denunciation() {
        // Create an endorsement denunciation and check if it is valid
        let (_slot, _keypair, s_endorsement_1, s_endorsement_2, _s_endorsement_3) =
            gen_endorsements_for_denunciation();
        let denunciation: Denunciation = (&s_endorsement_1, &s_endorsement_2).try_into().unwrap();

        assert_eq!(denunciation.is_for_endorsement(), true);
        assert_eq!(denunciation.is_valid().unwrap(), true);
    }

    #[test]
    fn test_endorsement_denunciation_invalid_1() {
        let (slot, keypair, s_endorsement_1, _s_endorsement_2, _s_endorsement_3) =
            gen_endorsements_for_denunciation();

        // Try to create a denunciation from 2 endorsements @ != index
        let endorsement_4 = Endorsement {
            slot,
            index: 9,
            endorsed_block: BlockId(Hash::compute_from("foo".as_bytes())),
        };
        let s_endorsement_4 =
            Endorsement::new_verifiable(endorsement_4, EndorsementSerializer::new(), &keypair)
                .unwrap();

        let denunciation = Denunciation::try_from((&s_endorsement_1, &s_endorsement_4));

        assert!(matches!(denunciation, Err(DenunciationError::InvalidInput)));

        // Try to create a denunciation from only 1 endorsement
        let denunciation = Denunciation::try_from((&s_endorsement_1, &s_endorsement_1));

        assert!(matches!(denunciation, Err(DenunciationError::InvalidInput)));
    }

    #[test]
    fn test_endorsement_denunciation_is_for() {
        let (slot, keypair, s_endorsement_1, s_endorsement_2, s_endorsement_3) =
            gen_endorsements_for_denunciation();

        let denunciation: Denunciation = (&s_endorsement_1, &s_endorsement_2).try_into().unwrap();

        assert_eq!(denunciation.is_for_endorsement(), true);
        assert_eq!(denunciation.is_valid().unwrap(), true);

        // Try to create a denunciation from 2 endorsements @ != index
        let endorsement_4 = Endorsement {
            slot,
            index: 9,
            endorsed_block: BlockId(Hash::compute_from("foo".as_bytes())),
        };
        let s_endorsement_4 =
            Endorsement::new_verifiable(endorsement_4, EndorsementSerializer::new(), &keypair)
                .unwrap();

        assert_eq!(
            denunciation
                .is_also_for_endorsement(&s_endorsement_4)
                .unwrap(),
            false
        );
        assert_eq!(
            denunciation
                .is_also_for_endorsement(&s_endorsement_3)
                .unwrap(),
            true
        );
        assert_eq!(denunciation.is_valid().unwrap(), true);
    }

    fn gen_block_headers_for_denunciation(
    ) -> (Slot, KeyPair, SecuredHeader, SecuredHeader, SecuredHeader) {
        let keypair = KeyPair::generate();

        let slot = Slot::new(2, 1);
        let parents_1: Vec<BlockId> = (0..THREAD_COUNT)
            .map(|i| BlockId(Hash::compute_from(&[i])))
            .collect();
        let parents_2: Vec<BlockId> = (0..THREAD_COUNT)
            .map(|i| BlockId(Hash::compute_from(&[i + 1])))
            .collect();
        let parents_3: Vec<BlockId> = (0..THREAD_COUNT)
            .map(|i| BlockId(Hash::compute_from(&[i + 2])))
            .collect();

        let endorsement_1 = Endorsement {
            slot: Slot::new(1, 1),
            index: 1,
            endorsed_block: BlockId(Hash::compute_from("blk1".as_bytes())),
        };
        let s_endorsement_1 =
            Endorsement::new_verifiable(endorsement_1, EndorsementSerializerLW::new(), &keypair)
                .unwrap();

        let block_header_1 = BlockHeader {
            slot,
            parents: parents_1,
            operation_merkle_root: Hash::compute_from("mno".as_bytes()),
            endorsements: vec![s_endorsement_1.clone()],
        };

        // create header
        let s_block_header_1 = BlockHeader::new_verifiable::<BlockHeaderSerializer, BlockId>(
            block_header_1,
            BlockHeaderSerializer::new(),
            &keypair,
        )
        .expect("error while producing block header");

        let block_header_2 = BlockHeader {
            slot,
            parents: parents_2,
            operation_merkle_root: Hash::compute_from("mno".as_bytes()),
            endorsements: vec![s_endorsement_1.clone()],
        };

        // create header
        let s_block_header_2 = BlockHeader::new_verifiable::<BlockHeaderSerializer, BlockId>(
            block_header_2,
            BlockHeaderSerializer::new(),
            &keypair,
        )
        .expect("error while producing block header");

        let block_header_3 = BlockHeader {
            slot,
            parents: parents_3,
            operation_merkle_root: Hash::compute_from("mno".as_bytes()),
            endorsements: vec![s_endorsement_1.clone()],
        };

        // create header
        let s_block_header_3 = BlockHeader::new_verifiable::<BlockHeaderSerializer, BlockId>(
            block_header_3,
            BlockHeaderSerializer::new(),
            &keypair,
        )
        .expect("error while producing block header");

        return (
            slot,
            keypair,
            s_block_header_1.clone(),
            s_block_header_2,
            s_block_header_3,
        );
    }

    #[test]
    fn test_block_header_denunciation() {
        // Create an block header denunciation and check if it is valid
        let (_slot, _keypair, s_block_header_1, s_block_header_2, s_block_header_3) =
            gen_block_headers_for_denunciation();
        let denunciation: Denunciation = (&s_block_header_1, &s_block_header_2).try_into().unwrap();

        assert_eq!(denunciation.is_for_block_header(), true);
        assert_eq!(denunciation.is_valid().unwrap(), true);
        assert_eq!(
            denunciation
                .is_also_for_block_header(&s_block_header_3)
                .unwrap(),
            true
        );
    }

    #[test]
    fn test_forge_invalid_denunciation() {
        let keypair = KeyPair::generate();
        let slot_1 = Slot::new(4, 2);
        let slot_2 = Slot::new(3, 7);

        let endorsement_1 = Endorsement {
            slot: slot_1,
            index: 0,
            endorsed_block: BlockId(Hash::compute_from("blk1".as_bytes())),
        };

        let s_endorsement_1: SecureShareEndorsement =
            Endorsement::new_verifiable(endorsement_1, EndorsementSerializer::new(), &keypair)
                .unwrap();

        let endorsement_2 = Endorsement {
            slot: slot_2,
            index: 0,
            endorsed_block: BlockId(Hash::compute_from("blk2".as_bytes())),
        };

        let s_endorsement_2: SecureShareEndorsement =
            Endorsement::new_verifiable(endorsement_2, EndorsementSerializer::new(), &keypair)
                .unwrap();

        // from an attacker - building manually a Denunciation object
        let de_forged_1 = Denunciation::Endorsement(EndorsementDenunciation {
            public_key: keypair.get_public_key(),
            slot: slot_1,
            index: 0,
            hash_1: *s_endorsement_1.id.get_hash(), // use only data from s_endorsement_1
            hash_2: *s_endorsement_1.id.get_hash(),
            signature_1: s_endorsement_1.signature,
            signature_2: s_endorsement_1.signature,
        });

        // hash_1 == hash_2 -> this is invalid
        assert_eq!(de_forged_1.is_valid().unwrap(), false);

        // from an attacker - building manually a Denunciation object
        let de_forged_2 = Denunciation::Endorsement(EndorsementDenunciation {
            public_key: keypair.get_public_key(),
            slot: slot_2,
            index: 0,
            hash_1: *s_endorsement_1.id.get_hash(),
            hash_2: *s_endorsement_2.id.get_hash(),
            signature_1: s_endorsement_1.signature,
            signature_2: s_endorsement_2.signature,
        });

        // An attacker uses an old s_endorsement_1 to forge a Denunciation object @ slot_2
        // This has to be detected if Denunciation are send via the network
        assert_eq!(de_forged_2.is_valid().unwrap(), false);
    }

    // SER / DER
    #[test]
    fn test_endorsement_denunciation_ser_der() {
        let (_, _, s_endorsement_1, s_endorsement_2, _) = gen_endorsements_for_denunciation();

        let denunciation = Denunciation::try_from((&s_endorsement_1, &s_endorsement_2)).unwrap();

        let mut buffer = Vec::new();
        let de_ser = EndorsementDenunciationSerializer::new();

        match denunciation {
            Denunciation::Endorsement(de) => {
                de_ser.serialize(&de, &mut buffer).unwrap();
                let de_der =
                    EndorsementDenunciationDeserializer::new(THREAD_COUNT, ENDORSEMENT_COUNT);

                let (rem, de_der_res) = de_der.deserialize::<DeserializeError>(&buffer).unwrap();

                assert_eq!(rem.is_empty(), true);
                assert_eq!(de, de_der_res);
            }
            Denunciation::BlockHeader(_) => {
                unimplemented!()
            }
        }
    }

    #[test]
    fn test_block_header_denunciation_ser_der() {
        let (_, _, s_block_header_1, s_block_header_2, _) = gen_block_headers_for_denunciation();
        let denunciation: Denunciation = (&s_block_header_1, &s_block_header_2).try_into().unwrap();

        let mut buffer = Vec::new();
        let de_ser = BlockHeaderDenunciationSerializer::new();

        match denunciation {
            Denunciation::Endorsement(_) => {
                unimplemented!()
            }
            Denunciation::BlockHeader(de) => {
                de_ser.serialize(&de, &mut buffer).unwrap();
                let de_der = BlockHeaderDenunciationDeserializer::new(THREAD_COUNT);

                let (rem, de_der_res) = de_der.deserialize::<DeserializeError>(&buffer).unwrap();

                assert_eq!(rem.is_empty(), true);
                assert_eq!(de, de_der_res);
            }
        }
    }

    #[test]
    fn test_denunciation_ser_der() {
        let (_, _, s_block_header_1, s_block_header_2, _) = gen_block_headers_for_denunciation();
        let denunciation: Denunciation = (&s_block_header_1, &s_block_header_2).try_into().unwrap();

        let mut buffer = Vec::new();
        let de_ser = DenunciationSerializer::new();

        de_ser.serialize(&denunciation, &mut buffer).unwrap();
        let de_der = DenunciationDeserializer::new(THREAD_COUNT, ENDORSEMENT_COUNT);

        let (rem, de_der_res) = de_der.deserialize::<DeserializeError>(&buffer).unwrap();

        assert_eq!(rem.is_empty(), true);
        assert_eq!(denunciation, de_der_res);

        let (_, _, s_endorsement_1, s_endorsement_2, _) = gen_endorsements_for_denunciation();
        let denunciation = Denunciation::try_from((&s_endorsement_1, &s_endorsement_2)).unwrap();
        buffer.clear();

        de_ser.serialize(&denunciation, &mut buffer).unwrap();
        let (rem, de_der_res) = de_der.deserialize::<DeserializeError>(&buffer).unwrap();
        assert_eq!(rem.is_empty(), true);
        assert_eq!(denunciation, de_der_res);
    }
}