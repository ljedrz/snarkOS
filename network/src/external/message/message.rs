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

use crate::external::message::MessageName;
use snarkos_errors::network::message::MessageError;

/// A trait used to abstract over network messages.
pub trait Message: Send + 'static {
    fn name() -> MessageName
    where
        Self: Sized;
    fn deserialize(bytes: Vec<u8>) -> Result<Self, MessageError>
    where
        Self: Sized;
    fn serialize(&self) -> Result<Vec<u8>, MessageError>;
}
