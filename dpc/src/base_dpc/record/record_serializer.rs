// Copyright (C) 2019-2020 Aleo Systems Inc.
// This file is part of the snarkOS library.

// The snarkOS library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkOS library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkOS library. If not, see <https://www.gnu.org/licenses/>.

use crate::base_dpc::{record::DPCRecord, record_payload::RecordPayload, BaseDPCComponents};
use snarkos_algorithms::encoding::Elligator2;
use snarkos_errors::dpc::DPCError;
use snarkos_models::{
    algorithms::{CommitmentScheme, CRH},
    curves::{AffineCurve, Group, MontgomeryModelParameters, PrimeField, ProjectiveCurve, TEModelParameters},
    dpc::{DPCComponents, Record, RecordSerializerScheme},
};
use snarkos_utilities::{bits_to_bytes, bytes_to_bits, to_bytes, BigInteger, FromBytes, ToBytes};

use itertools::Itertools;
use std::marker::PhantomData;

/// Encode a base field element bytes to a group representation
pub fn encode_to_group<P: MontgomeryModelParameters + TEModelParameters, G: Group + ProjectiveCurve>(
    x_bytes: &[u8],
) -> Result<(<G as ProjectiveCurve>::Affine, bool), DPCError> {
    // TODO (howardwu): Remove this hardcoded value and use BaseField's size in bits to pad length.
    let mut bytes = x_bytes.to_vec();
    while bytes.len() < 32 {
        bytes.push(0)
    }

    let input = P::BaseField::read(&bytes[..])?;
    let (output, fq_high) = Elligator2::<P, G>::encode(&input)?;
    Ok((output, fq_high))
}

/// Decode a group into the byte representation of a base field element
pub fn decode_from_group<P: MontgomeryModelParameters + TEModelParameters, G: Group + ProjectiveCurve>(
    affine: <G as ProjectiveCurve>::Affine,
    fq_high: bool,
) -> Result<Vec<u8>, DPCError> {
    let output = Elligator2::<P, G>::decode(&affine, fq_high)?;
    Ok(to_bytes![output]?)
}

pub struct DeserializedRecord<C: BaseDPCComponents> {
    pub serial_number_nonce: <C::SerialNumberNonceCRH as CRH>::Output,
    pub commitment_randomness: <C::RecordCommitment as CommitmentScheme>::Randomness,
    pub birth_program_id: Vec<u8>,
    pub death_program_id: Vec<u8>,
    pub payload: RecordPayload,
    pub value: u64,
}

pub struct RecordSerializer<
    C: BaseDPCComponents,
    P: MontgomeryModelParameters + TEModelParameters,
    G: Group + ProjectiveCurve,
>(PhantomData<C>, PhantomData<P>, PhantomData<G>);

impl<C: BaseDPCComponents, P: MontgomeryModelParameters + TEModelParameters, G: Group + ProjectiveCurve>
    RecordSerializerScheme for RecordSerializer<C, P, G>
{
    type DeserializedRecord = DeserializedRecord<C>;
    type Group = G;
    type InnerField = <C as DPCComponents>::InnerField;
    type OuterField = <C as DPCComponents>::OuterField;
    type Parameters = P;
    type Record = DPCRecord<C>;

    /// Records are serialized in a specialized format to be space-saving.
    ///
    /// Serialized element 1 - [ Serial number nonce ]
    /// Serialized element 2 - [ Commitment randomness ]
    /// Serialized element 3 - [ Birth program id (part 1) ]
    /// Serialized element 4 - [ Death program id (part 1) ]
    /// Serialized element 5 - [ Birth program id (part 2) || Death program id (part 2) ]
    /// Serialized element 6 - [ Payload (part 1) || 1 ]
    /// Serialized element 7 - [ 1 || Sign high bits (7 bits) || Value || Payload (part 2) ]
    ///
    fn serialize(record: &Self::Record) -> Result<(Vec<Self::Group>, bool), DPCError> {
        // Assumption 1 - The scalar field bit size must be strictly less than the base field bit size
        // for the logic below to work correctly.
        assert!(Self::SCALAR_FIELD_BITSIZE < Self::INNER_FIELD_BITSIZE);

        // Assumption 2 - this implementation assumes the outer field bit size is larger than
        // the data field bit size by at most one additional scalar field bit size.
        assert!((Self::OUTER_FIELD_BITSIZE - Self::DATA_ELEMENT_BITSIZE) <= Self::DATA_ELEMENT_BITSIZE);

        // Assumption 3 - this implementation assumes the remainder of two outer field bit sizes
        // can fit within one data field element's bit size.
        assert!((2 * (Self::OUTER_FIELD_BITSIZE - Self::DATA_ELEMENT_BITSIZE)) <= Self::DATA_ELEMENT_BITSIZE);

        // Assumption 4 - this implementation assumes the payload and value may be zero values.
        // As such, to ensure the values are non-zero for encoding and decoding, we explicitly
        // reserve the MSB of the data field element's valid bitsize and set the bit to 1.
        assert_eq!(Self::PAYLOAD_ELEMENT_BITSIZE, Self::DATA_ELEMENT_BITSIZE - 1);

        // This element needs to be represented in the constraint field; its bits and the number of elements
        // are calculated early, so that the storage vectors can be pre-allocated.
        let payload = record.payload();
        let payload_bytes = to_bytes![payload]?;
        let payload_bits_count = payload_bytes.len() * 8;
        let payload_bits = bytes_to_bits(&payload_bytes);
        let num_payload_elements = payload_bits_count / Self::PAYLOAD_ELEMENT_BITSIZE;

        // Create the vector for storing data elements.

        let mut data_elements = Vec::with_capacity(5 + num_payload_elements + 2);
        let mut data_high_bits = Vec::with_capacity(5 + num_payload_elements);

        // These elements are already in the constraint field.

        let serial_number_nonce = record.serial_number_nonce();
        let serial_number_nonce_encoded =
            <Self::Group as ProjectiveCurve>::Affine::from_random_bytes(&to_bytes![serial_number_nonce]?.to_vec())
                .unwrap();

        data_elements.push(serial_number_nonce_encoded);
        data_high_bits.push(false);

        assert_eq!(data_elements.len(), 1);
        assert_eq!(data_high_bits.len(), 1);

        // These elements need to be represented in the constraint field.

        let commitment_randomness = record.commitment_randomness();
        let birth_program_id = record.birth_program_id();
        let death_program_id = record.death_program_id();
        let value = record.value();

        // Process commitment_randomness. (Assumption 1 applies)

        let (encoded_commitment_randomness, sign_high) =
            encode_to_group::<Self::Parameters, Self::Group>(&to_bytes![commitment_randomness]?[..])?;
        data_elements.push(encoded_commitment_randomness);
        data_high_bits.push(sign_high);

        assert_eq!(data_elements.len(), 2);
        assert_eq!(data_high_bits.len(), 2);

        // Process birth_program_id and death_program_id. (Assumption 2 and 3 applies)

        let birth_program_id_biginteger = Self::OuterField::read(&birth_program_id[..])?.into_repr();
        let death_program_id_biginteger = Self::OuterField::read(&death_program_id[..])?.into_repr();

        let mut birth_program_id_bits = Vec::with_capacity(Self::INNER_FIELD_BITSIZE);
        let mut death_program_id_bits = Vec::with_capacity(Self::INNER_FIELD_BITSIZE);
        let mut birth_program_id_remainder_bits =
            Vec::with_capacity(Self::OUTER_FIELD_BITSIZE - Self::DATA_ELEMENT_BITSIZE);
        let mut death_program_id_remainder_bits =
            Vec::with_capacity(Self::OUTER_FIELD_BITSIZE - Self::DATA_ELEMENT_BITSIZE);

        for i in 0..Self::DATA_ELEMENT_BITSIZE {
            birth_program_id_bits.push(birth_program_id_biginteger.get_bit(i));
            death_program_id_bits.push(death_program_id_biginteger.get_bit(i));
        }

        // (Assumption 2 applies)
        for i in Self::DATA_ELEMENT_BITSIZE..Self::OUTER_FIELD_BITSIZE {
            birth_program_id_remainder_bits.push(birth_program_id_biginteger.get_bit(i));
            death_program_id_remainder_bits.push(death_program_id_biginteger.get_bit(i));
        }
        birth_program_id_remainder_bits.append(&mut death_program_id_remainder_bits);

        // (Assumption 3 applies)

        let (encoded_birth_program_id, sign_high) =
            encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&birth_program_id_bits)[..])?;
        drop(birth_program_id_bits);
        data_elements.push(encoded_birth_program_id);
        data_high_bits.push(sign_high);

        let (encoded_death_program_id, sign_high) =
            encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&death_program_id_bits)[..])?;
        drop(death_program_id_bits);
        data_elements.push(encoded_death_program_id);
        data_high_bits.push(sign_high);

        let (encoded_birth_program_id_remainder, sign_high) =
            encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&birth_program_id_remainder_bits)[..])?;
        drop(birth_program_id_remainder_bits);
        data_elements.push(encoded_birth_program_id_remainder);
        data_high_bits.push(sign_high);

        assert_eq!(data_elements.len(), 5);
        assert_eq!(data_high_bits.len(), 5);

        // Process payload.

        let mut payload_field_bits = Vec::with_capacity(Self::PAYLOAD_ELEMENT_BITSIZE + 1);

        for (i, bit) in payload_bits.enumerate() {
            payload_field_bits.push(bit);

            if (i > 0) && ((i + 1) % Self::PAYLOAD_ELEMENT_BITSIZE == 0) {
                // (Assumption 4)
                payload_field_bits.push(true);
                let (encoded_payload_field, sign_high) =
                    encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&payload_field_bits)[..])?;

                data_elements.push(encoded_payload_field);
                data_high_bits.push(sign_high);

                payload_field_bits.clear();
            }
        }

        assert_eq!(data_elements.len(), 5 + num_payload_elements);
        assert_eq!(data_high_bits.len(), 5 + num_payload_elements);

        // Process payload remainder and value.

        // Determine if value can fit in current payload_field_bits.
        let value_does_not_fit =
            (payload_field_bits.len() + data_high_bits.len() + (std::mem::size_of_val(&value) * 8))
                > Self::PAYLOAD_ELEMENT_BITSIZE;

        if value_does_not_fit {
            // (Assumption 4)
            payload_field_bits.push(true);

            let (encoded_payload_field, fq_high) =
                encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&payload_field_bits)[..])?;

            data_elements.push(encoded_payload_field);
            data_high_bits.push(fq_high);

            payload_field_bits.clear();
        }

        assert_eq!(
            data_elements.len(),
            5 + num_payload_elements + (value_does_not_fit as usize)
        );

        // Append the value bits and create the final base element.
        let value_bits = bytes_to_bits(&to_bytes![value]?).collect();

        // (Assumption 4)
        let final_element = [vec![true], data_high_bits, value_bits, payload_field_bits].concat();
        let (encoded_final_element, final_sign_high) =
            encode_to_group::<Self::Parameters, Self::Group>(&bits_to_bytes(&final_element)[..])?;

        data_elements.push(encoded_final_element);

        assert_eq!(
            data_elements.len(),
            5 + num_payload_elements + (value_does_not_fit as usize) + 1
        );

        // Compute the output group elements.

        let mut output = Vec::with_capacity(data_elements.len());
        for element in data_elements.iter() {
            output.push(element.into_projective());
        }

        Ok((output, final_sign_high))
    }

    /// Deserialize and return the record components
    fn deserialize(
        serialized_record: Vec<Self::Group>,
        final_sign_high: bool,
    ) -> Result<Self::DeserializedRecord, DPCError> {
        let remainder_size = Self::OUTER_FIELD_BITSIZE - Self::DATA_ELEMENT_BITSIZE;

        // Extract the fq_bits
        let final_element = &serialized_record[serialized_record.len() - 1];
        let final_element_bytes =
            decode_from_group::<Self::Parameters, Self::Group>(final_element.into_affine(), final_sign_high)?;
        let final_element_bits = bytes_to_bits(&final_element_bytes).collect::<Vec<_>>();

        let fq_high_bits = &final_element_bits[1..serialized_record.len()];

        // Deserialize serial number nonce

        let (serial_number_nonce, _) = &(serialized_record[0], fq_high_bits[0]);
        let serial_number_nonce_bytes = to_bytes![serial_number_nonce.into_affine().to_x_coordinate()]?;
        let serial_number_nonce = <C::SerialNumberNonceCRH as CRH>::Output::read(&serial_number_nonce_bytes[..])?;

        // Deserialize commitment randomness

        let (commitment_randomness, commitment_randomness_fq_high) = &(serialized_record[1], fq_high_bits[1]);
        let commitment_randomness_bytes = decode_from_group::<Self::Parameters, Self::Group>(
            commitment_randomness.into_affine(),
            *commitment_randomness_fq_high,
        )?;
        let commitment_randomness_bits = &bytes_to_bits(&commitment_randomness_bytes)
            .take(Self::DATA_ELEMENT_BITSIZE)
            .collect::<Vec<_>>();
        let commitment_randomness = <C::RecordCommitment as CommitmentScheme>::Randomness::read(
            &bits_to_bytes(commitment_randomness_bits)[..],
        )?;

        // Deserialize birth and death programs

        let (birth_program_id, birth_program_id_sign_high) = &(serialized_record[2], fq_high_bits[2]);
        let birth_program_id_bytes = decode_from_group::<Self::Parameters, Self::Group>(
            birth_program_id.into_affine(),
            *birth_program_id_sign_high,
        )?;

        let (death_program_id, death_program_id_sign_high) = &(serialized_record[3], fq_high_bits[3]);
        let death_program_id_bytes = decode_from_group::<Self::Parameters, Self::Group>(
            death_program_id.into_affine(),
            *death_program_id_sign_high,
        )?;

        let (program_id_remainder, program_id_sign_high) = &(serialized_record[4], fq_high_bits[4]);
        let program_id_remainder_bytes = decode_from_group::<Self::Parameters, Self::Group>(
            program_id_remainder.into_affine(),
            *program_id_sign_high,
        )?;

        let mut birth_program_id_bits = bytes_to_bits(&birth_program_id_bytes)
            .take(Self::DATA_ELEMENT_BITSIZE)
            .collect::<Vec<_>>();
        let mut death_program_id_bits = bytes_to_bits(&death_program_id_bytes)
            .take(Self::DATA_ELEMENT_BITSIZE)
            .collect::<Vec<_>>();

        let mut program_id_remainder_bits = bytes_to_bits(&program_id_remainder_bytes);
        birth_program_id_bits.extend(program_id_remainder_bits.by_ref().take(remainder_size));
        death_program_id_bits.extend(program_id_remainder_bits.take(remainder_size));

        let birth_program_id = bits_to_bytes(&birth_program_id_bits);
        let death_program_id = bits_to_bytes(&death_program_id_bits);

        // Deserialize the value

        let value_start = serialized_record.len();
        let value_end = value_start + (std::mem::size_of_val(&<Self::Record as Record>::Value::default()) * 8);
        let value: <Self::Record as Record>::Value =
            FromBytes::read(&bits_to_bytes(&final_element_bits[value_start..value_end])[..])?;

        // Deserialize payload

        let mut payload_bits = vec![];
        for (element, fq_high) in serialized_record[5..serialized_record.len() - 1]
            .iter()
            .zip_eq(&fq_high_bits[5..])
        {
            let element_bytes = decode_from_group::<Self::Parameters, Self::Group>(element.into_affine(), *fq_high)?;
            payload_bits.extend(bytes_to_bits(&element_bytes).take(Self::PAYLOAD_ELEMENT_BITSIZE));
        }
        payload_bits.extend_from_slice(&final_element_bits[value_end..]);

        let payload = RecordPayload::read(&bits_to_bytes(&payload_bits)[..])?;

        Ok(DeserializedRecord {
            serial_number_nonce,
            commitment_randomness,
            birth_program_id,
            death_program_id,
            payload,
            value,
        })
    }
}
