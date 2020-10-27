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

//! Implementation of private RPC endpoints that require authentication.
//!
//! See [ProtectedRpcFunctions](../trait.ProtectedRpcFunctions.html) for documentation of private endpoints.

use crate::{rpc_trait::ProtectedRpcFunctions, rpc_types::*, RpcImpl};
use snarkos_dpc::base_dpc::{
    encrypted_record::EncryptedRecord,
    instantiated::{Components, InstantiatedDPC},
    record::DPCRecord,
    record_encryption::RecordEncryption,
    record_payload::RecordPayload,
};
use snarkos_errors::rpc::RpcError;
use snarkos_models::{
    algorithms::CRH,
    dpc::{DPCComponents, Record},
    objects::AccountScheme,
};
use snarkos_objects::{Account, AccountAddress, AccountPrivateKey, AccountViewKey};
use snarkos_utilities::{
    bytes::{FromBytes, ToBytes},
    to_bytes,
};

use jsonrpc_http_server::jsonrpc_core::{IoDelegate, MetaIoHandler, Params, Value};
use rand::{thread_rng, Rng};
use std::{str::FromStr, sync::Arc};

type JsonRPCError = jsonrpc_core::Error;

/// The following `*_protected` functions wrap an authentication check around sensitive functions
/// before being exposed as an RPC endpoint
impl RpcImpl {
    /// Validate the authentication header in the request metadata
    pub fn validate_auth(&self, meta: Meta) -> Result<(), JsonRPCError> {
        if let Some(credentials) = &self.credentials {
            let auth = meta.auth.unwrap_or_else(String::new);
            let basic_auth_encoding = format!(
                "Basic {}",
                base64::encode(format!("{}:{}", credentials.username, credentials.password))
            );

            if basic_auth_encoding != auth {
                return Err(JsonRPCError::invalid_params("Authentication Error"));
            }
        }

        Ok(())
    }

    /// Wrap authentication around `create_raw_transaction`
    pub fn create_raw_transaction_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        let value = match params {
            Params::Array(arr) => arr,
            _ => return Err(JsonRPCError::invalid_request()),
        };

        let val: TransactionInputs = serde_json::from_value(value[0].clone())
            .map_err(|e| JsonRPCError::invalid_params(format!("Invalid params: {}.", e)))?;

        match self.create_raw_transaction(val) {
            Ok(result) => Ok(serde_json::to_value(result).expect("transaction output serialization failed")),
            Err(err) => Err(JsonRPCError::invalid_params(err.to_string())),
        }
    }

    /// Wrap authentication around `get_record_commitment_count`
    pub fn get_record_commitment_count_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        params.expect_no_params()?;

        match self.get_record_commitment_count() {
            Ok(num_record_commitments) => Ok(Value::from(num_record_commitments)),
            Err(_) => Err(JsonRPCError::invalid_request()),
        }
    }

    /// Wrap authentication around `get_record_commitments`
    pub fn get_record_commitments_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        params.expect_no_params()?;

        match self.get_record_commitments() {
            Ok(record_commitments) => Ok(Value::from(record_commitments)),
            Err(_) => Err(JsonRPCError::invalid_request()),
        }
    }

    /// Wrap authentication around `get_raw_record`
    pub fn get_raw_record_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        let value = match params {
            Params::Array(arr) => arr,
            _ => return Err(JsonRPCError::invalid_request()),
        };

        if value.len() != 1 {
            return Err(JsonRPCError::invalid_params(format!(
                "invalid length {}, expected 1 element",
                value.len()
            )));
        }

        let record_commitment: String = serde_json::from_value(value[0].clone())
            .map_err(|e| JsonRPCError::invalid_params(format!("Invalid params: {}.", e)))?;

        match self.get_raw_record(record_commitment) {
            Ok(record) => Ok(Value::from(record)),
            Err(err) => Err(JsonRPCError::invalid_params(err.to_string())),
        }
    }

    /// Wrap authentication around `decode_record`
    pub fn decode_record_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        let value = match params {
            Params::Array(arr) => arr,
            _ => return Err(JsonRPCError::invalid_request()),
        };

        if value.len() != 1 {
            return Err(JsonRPCError::invalid_params(format!(
                "invalid length {}, expected 1 element",
                value.len()
            )));
        }

        let record_bytes: String = serde_json::from_value(value[0].clone())
            .map_err(|e| JsonRPCError::invalid_params(format!("Invalid params: {}.", e)))?;

        match self.decode_record(record_bytes) {
            Ok(record) => Ok(serde_json::to_value(record).expect("record deserialization failed")),
            Err(err) => Err(JsonRPCError::invalid_params(err.to_string())),
        }
    }

    /// Wrap authentication around `decrypt_record`
    pub fn decrypt_record_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        let value = match params {
            Params::Array(arr) => arr,
            _ => return Err(JsonRPCError::invalid_request()),
        };

        let decrypt_record_input: DecryptRecordInput = serde_json::from_value(value[0].clone())
            .map_err(|e| JsonRPCError::invalid_params(format!("Invalid params: {}.", e)))?;

        match self.decrypt_record(decrypt_record_input) {
            Ok(result) => Ok(serde_json::to_value(result).expect("record serialization failed")),
            Err(err) => Err(JsonRPCError::invalid_params(err.to_string())),
        }
    }

    /// Wrap authentication around `create_account`
    pub fn create_account_protected(&self, params: Params, meta: Meta) -> Result<Value, JsonRPCError> {
        self.validate_auth(meta)?;

        params.expect_no_params()?;

        match self.create_account() {
            Ok(account) => Ok(serde_json::to_value(account).expect("account serialization failed")),
            Err(err) => Err(JsonRPCError::invalid_params(err.to_string())),
        }
    }

    /// Expose the protected functions as RPC enpoints
    pub fn add_protected(&self, io: &mut MetaIoHandler<Meta>) {
        let mut d = IoDelegate::<Self, Meta>::new(Arc::new(self.clone()));

        d.add_method_with_meta("createrawtransaction", Self::create_raw_transaction_protected);
        d.add_method_with_meta("decoderecord", Self::decode_record_protected);
        d.add_method_with_meta("decryptrecord", Self::decrypt_record_protected);
        d.add_method_with_meta("getrecordcommitmentcount", Self::get_record_commitment_count_protected);
        d.add_method_with_meta("getrecordcommitments", Self::get_record_commitments_protected);
        d.add_method_with_meta("getrawrecord", Self::get_raw_record_protected);
        d.add_method_with_meta("createaccount", Self::create_account_protected);

        io.extend_with(d)
    }
}

/// Functions that are sensitive and need to be protected with authentication.
/// The authentication logic is defined in `validate_auth`
impl ProtectedRpcFunctions for RpcImpl {
    /// Generate a new account private key, account view key, and account address.
    fn create_account(&self) -> Result<RpcAccount, RpcError> {
        let rng = &mut thread_rng();

        let account = Account::<Components>::new(
            self.parameters.account_signature_parameters(),
            self.parameters.account_commitment_parameters(),
            self.parameters.account_encryption_parameters(),
            rng,
        )?;

        let view_key = AccountViewKey::<Components>::from_private_key(
            self.parameters.account_signature_parameters(),
            self.parameters.account_commitment_parameters(),
            &account.private_key,
        )?;

        Ok(RpcAccount {
            private_key: account.private_key.to_string(),
            view_key: view_key.to_string(),
            address: account.address.to_string(),
        })
    }

    /// Create a new transaction, returning the encoded transaction and the new records.
    fn create_raw_transaction(
        &self,
        transaction_input: TransactionInputs,
    ) -> Result<CreateRawTransactionOuput, RpcError> {
        let rng = &mut thread_rng();

        assert!(!transaction_input.old_records.is_empty());
        assert!(transaction_input.old_records.len() <= Components::NUM_OUTPUT_RECORDS);
        assert!(!transaction_input.old_account_private_keys.is_empty());
        assert!(transaction_input.old_account_private_keys.len() <= Components::NUM_OUTPUT_RECORDS);
        assert!(!transaction_input.recipients.is_empty());
        assert!(transaction_input.recipients.len() <= Components::NUM_OUTPUT_RECORDS);

        // Fetch birth/death programs
        let program_vk_hash = self
            .parameters
            .system_parameters
            .program_verification_key_crh
            .hash(&to_bytes![
                self.parameters.noop_program_snark_parameters.verification_key
            ]?)?;
        let program_vk_hash_bytes = to_bytes![program_vk_hash]?;

        let program_id = program_vk_hash_bytes;
        let new_birth_program_ids = vec![program_id.clone(); Components::NUM_OUTPUT_RECORDS];
        let new_death_program_ids = vec![program_id.clone(); Components::NUM_OUTPUT_RECORDS];

        // Decode old records
        let mut old_records = Vec::with_capacity(transaction_input.old_records.len());
        for record_string in transaction_input.old_records {
            let record_bytes = hex::decode(record_string)?;
            old_records.push(DPCRecord::<Components>::read(&record_bytes[..])?);
        }

        let mut old_account_private_keys = Vec::with_capacity(transaction_input.old_account_private_keys.len());
        for private_key_string in transaction_input.old_account_private_keys {
            old_account_private_keys.push(AccountPrivateKey::<Components>::from_str(&private_key_string)?);
        }

        let sn_randomness: [u8; 32] = rng.gen();
        // Fill any unused old_record indices with dummy records
        while old_records.len() < Components::NUM_OUTPUT_RECORDS {
            let old_sn_nonce = self
                .parameters
                .system_parameters
                .serial_number_nonce
                .hash(&sn_randomness)?;

            let private_key = old_account_private_keys[0].clone();
            let address = AccountAddress::<Components>::from_private_key(
                self.parameters.account_signature_parameters(),
                self.parameters.account_commitment_parameters(),
                self.parameters.account_encryption_parameters(),
                &private_key,
            )?;

            let dummy_record = InstantiatedDPC::generate_record(
                self.parameters.system_parameters.clone(),
                old_sn_nonce,
                address,
                true, // The input record is dummy
                0,
                RecordPayload::default(),
                program_id.clone(),
                program_id.clone(),
                rng,
            )?;

            old_records.push(dummy_record);
            old_account_private_keys.push(private_key);
        }

        assert_eq!(old_records.len(), Components::NUM_INPUT_RECORDS);
        assert_eq!(old_account_private_keys.len(), Components::NUM_INPUT_RECORDS);

        // Decode new recipient data
        let mut new_record_owners = Vec::with_capacity(Components::NUM_OUTPUT_RECORDS);
        let mut new_is_dummy_flags = Vec::with_capacity(Components::NUM_OUTPUT_RECORDS);
        let mut new_values = Vec::with_capacity(Components::NUM_OUTPUT_RECORDS);
        for recipient in transaction_input.recipients {
            new_record_owners.push(AccountAddress::<Components>::from_str(&recipient.address)?);
            new_is_dummy_flags.push(false);
            new_values.push(recipient.amount);
        }

        // Fill any unused new_record indices with dummy output values
        while new_record_owners.len() < Components::NUM_OUTPUT_RECORDS {
            new_record_owners.push(new_record_owners[0].clone());
            new_is_dummy_flags.push(true);
            new_values.push(0);
        }

        assert_eq!(new_record_owners.len(), Components::NUM_OUTPUT_RECORDS);
        assert_eq!(new_is_dummy_flags.len(), Components::NUM_OUTPUT_RECORDS);
        assert_eq!(new_values.len(), Components::NUM_OUTPUT_RECORDS);

        // Default record payload
        let new_payloads = vec![RecordPayload::default(); Components::NUM_OUTPUT_RECORDS];

        // Decode memo
        let mut memo = [0u8; 32];
        if let Some(memo_string) = transaction_input.memo {
            if let Ok(bytes) = hex::decode(memo_string) {
                bytes.write(&mut memo[..])?;
            }
        }

        // If the request did not specify a valid memo, generate one from random
        if memo == [0u8; 32] {
            memo = rng.gen();
        }

        // Because this is a computationally heavy endpoint, we open a
        // new secondary storage instance to prevent storage bottle-necking.
        let storage = self.new_secondary_storage_instance()?;

        // Generate transaction
        let (records, transaction) = self.consensus.create_transaction(
            &self.parameters,
            old_records,
            old_account_private_keys,
            new_record_owners,
            new_birth_program_ids,
            new_death_program_ids,
            new_is_dummy_flags,
            new_values,
            new_payloads,
            memo,
            &storage,
            rng,
        )?;

        let encoded_transaction = hex::encode(to_bytes![transaction]?);
        let mut encoded_records = Vec::with_capacity(records.len());
        for record in records {
            encoded_records.push(hex::encode(to_bytes![record]?));
        }

        Ok(CreateRawTransactionOuput {
            encoded_transaction,
            encoded_records,
        })
    }

    /// Returns the number of record commitments that are stored on the full node.
    fn get_record_commitment_count(&self) -> Result<usize, RpcError> {
        self.storage.catch_up_secondary(false)?;
        let record_commitments = self.storage.get_record_commitments(None)?;

        Ok(record_commitments.len())
    }

    /// Returns a list of record commitments that are stored on the full node.
    fn get_record_commitments(&self) -> Result<Vec<String>, RpcError> {
        self.storage.catch_up_secondary(false)?;
        let record_commitments = self.storage.get_record_commitments(Some(100))?;
        let record_commitment_strings: Vec<String> = record_commitments.iter().map(hex::encode).collect();

        Ok(record_commitment_strings)
    }

    /// Returns the hex encoded bytes of a record from its record commitment
    fn get_raw_record(&self, record_commitment: String) -> Result<String, RpcError> {
        match self
            .storage
            .get_record::<DPCRecord<Components>>(&hex::decode(record_commitment)?)?
        {
            Some(record) => {
                let record_bytes = to_bytes![record]?;
                Ok(hex::encode(record_bytes))
            }
            None => Ok("Record not found".to_string()),
        }
    }

    /// Decrypts the record ciphertext and returns the hex encoded bytes of the record.
    fn decrypt_record(&self, decryption_input: DecryptRecordInput) -> Result<String, RpcError> {
        // Read the encrypted_record
        let encrypted_record_bytes = hex::decode(decryption_input.encrypted_record)?;
        let encrypted_record = EncryptedRecord::<Components>::read(&encrypted_record_bytes[..])?;

        // Read the view key
        let account_view_key = AccountViewKey::<Components>::from_str(&decryption_input.account_view_key)?;

        // Decrypt the record ciphertext
        let record =
            RecordEncryption::decrypt_record(&self.parameters.system_parameters, &account_view_key, &encrypted_record)?;
        let record_bytes = to_bytes![record]?;

        Ok(hex::encode(record_bytes))
    }

    /// Returns information about a record from serialized record bytes.
    fn decode_record(&self, record_bytes: String) -> Result<RecordInfo, RpcError> {
        let record_bytes = hex::decode(record_bytes)?;
        let record = DPCRecord::<Components>::read(&record_bytes[..])?;

        let owner = record.owner().to_string();
        let payload = RPCRecordPayload {
            payload: hex::encode(to_bytes![record.payload()]?),
        };
        let birth_program_id = hex::encode(record.birth_program_id());
        let death_program_id = hex::encode(record.death_program_id());
        let serial_number_nonce = hex::encode(to_bytes![record.serial_number_nonce()]?);
        let commitment = hex::encode(to_bytes![record.commitment()]?);
        let commitment_randomness = hex::encode(to_bytes![record.commitment_randomness()]?);

        Ok(RecordInfo {
            owner,
            is_dummy: record.is_dummy(),
            value: record.value(),
            payload,
            birth_program_id,
            death_program_id,
            serial_number_nonce,
            commitment,
            commitment_randomness,
        })
    }
}
