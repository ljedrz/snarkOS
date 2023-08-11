// Copyright (C) 2019-2023 Aleo Systems Inc.
// This file is part of the snarkOS library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:
// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;

/// Different consensus-related issues claimed by other committee members.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum NarwhalErrorKind {
    /// The round in the batch proposal was too old or in the future.
    InvalidBatchProposalRound(u64), // peer's round
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct NarwhalError {
    pub kind: NarwhalErrorKind,
}

impl From<NarwhalErrorKind> for NarwhalError {
    fn from(kind: NarwhalErrorKind) -> Self {
        Self { kind }
    }
}

impl EventTrait for NarwhalError {
    /// Returns the event name.
    #[inline]
    fn name(&self) -> &'static str {
        "NarwhalError"
    }

    /// Serializes the event into the buffer.
    #[inline]
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        Ok(bincode::serialize_into(writer, &self.kind)?)
    }

    /// Deserializes the given buffer into an event.
    #[inline]
    fn deserialize(bytes: BytesMut) -> Result<Self> {
        if let Ok(kind) = bincode::deserialize_from(&mut bytes.reader()) {
            Ok(Self { kind })
        } else {
            bail!("Invalid 'NarwhalError' event");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{event::EventTrait, NarwhalError, NarwhalErrorKind};
    use bytes::{BufMut, BytesMut};

    #[test]
    fn serialize_deserialize() {
        let all_kinds = vec![NarwhalErrorKind::BatchProposalExpired];

        for kind in all_kinds.iter() {
            let disconnect = NarwhalError::from(*kind);
            let mut buf = BytesMut::default().writer();
            NarwhalError::serialize(&disconnect, &mut buf).unwrap();

            let disconnect = NarwhalError::deserialize(buf.get_ref().clone()).unwrap();
            assert_eq!(kind, &disconnect.kind);
        }
    }

    #[test]
    #[should_panic(expected = "Invalid 'NarwhalError' event")]
    fn deserializing_invalid_data_panics() {
        let mut buf = BytesMut::default().writer();
        bincode::serialize_into(&mut buf, "not a NarwhalErrorKind-value").unwrap();
        let _disconnect = Disconnect::deserialize(buf.get_ref().clone()).unwrap();
    }
}
