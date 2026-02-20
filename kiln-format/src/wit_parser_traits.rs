//! Trait implementations for WIT parser types

use super::wit_parser_types::*;
use wrt_foundation::{
    traits::{Checksummable, FromBytes, ToBytes, ReadStream, WriteStream},
    verification::Checksum,
    MemoryProvider, BoundedVec,
};
use wrt_error::Result;
use wrt_error::{Error, ErrorCategory};
use core::default::Default;

// ===== Default implementations =====

impl Default for WitImport {
    fn default() -> Self {
        Self {
            name: WitBoundedString::default(),
            item: WitItem::default(),
        }
    }
}

impl Default for WitExport {
    fn default() -> Self {
        Self {
            name: WitBoundedString::default(),
            item: WitItem::default(),
        }
    }
}

impl Default for WitItem {
    fn default() -> Self {
        WitItem::Function(WitFunction::default())
    }
}

impl Default for WitFunction {
    fn default() -> Self {
        Self {
            name: WitBoundedString::default(),
            params: BoundedVec::new(),
            results: BoundedVec::new(),
            is_async: false,
        }
    }
}

impl Default for WitParam {
    fn default() -> Self {
        Self {
            name: WitBoundedStringSmall::default(),
            ty: WitType::default(),
        }
    }
}

impl Default for WitResult {
    fn default() -> Self {
        Self {
            name: None,
            ty: WitType::default(),
        }
    }
}

impl Default for WitInstance {
    fn default() -> Self {
        Self {
            interface_name: WitBoundedString::default(),
            args: BoundedVec::new(WitProvider::default()).unwrap_or_default(),
        }
    }
}

impl Default for WitInstanceArg {
    fn default() -> Self {
        Self {
            name: WitBoundedStringSmall::default(),
            value: WitValue::default(),
        }
    }
}

impl Default for WitValue {
    fn default() -> Self {
        WitValue::Type(WitType::default())
    }
}

impl Default for WitTypeDef {
    fn default() -> Self {
        Self {
            name: WitBoundedString::default(),
            ty: WitType::default(),
            is_resource: false,
        }
    }
}

impl Default for WitType {
    fn default() -> Self {
        WitType::Bool
    }
}

impl Default for WitRecord {
    fn default() -> Self {
        Self {
            fields: BoundedVec::new(),
        }
    }
}

impl Default for WitRecordField {
    fn default() -> Self {
        Self {
            name: WitBoundedStringSmall::default(),
            ty: WitType::default(),
        }
    }
}

impl Default for WitVariant {
    fn default() -> Self {
        Self {
            cases: BoundedVec::new(),
        }
    }
}

impl Default for WitVariantCase {
    fn default() -> Self {
        Self {
            name: WitBoundedStringSmall::default(),
            ty: None,
        }
    }
}

impl Default for WitEnum {
    fn default() -> Self {
        Self {
            cases: BoundedVec::new(),
        }
    }
}

impl Default for WitFlags {
    fn default() -> Self {
        Self {
            flags: BoundedVec::new(),
        }
    }
}

// ===== Eq implementations =====

impl Eq for WitImport {}
impl Eq for WitExport {}
impl Eq for WitItem {}
impl Eq for WitFunction {}
impl Eq for WitParam {}
impl Eq for WitResult {}
impl Eq for WitInstance {}
impl Eq for WitInstanceArg {}
impl Eq for WitValue {}
impl Eq for WitTypeDef {}
impl Eq for WitType {}
impl Eq for WitRecord {}
impl Eq for WitRecordField {}
impl Eq for WitVariant {}
impl Eq for WitVariantCase {}
impl Eq for WitEnum {}
impl Eq for WitFlags {}

// ===== Checksummable implementations =====

impl Checksummable for WitImport {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.item.update_checksum(checksum;
    }
}

impl Checksummable for WitExport {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.item.update_checksum(checksum;
    }
}

impl Checksummable for WitItem {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            WitItem::Function(f) => {
                checksum.update(0;
                f.update_checksum(checksum;
            }
            WitItem::Interface(i) => {
                checksum.update(1;
                i.update_checksum(checksum;
            }
            WitItem::Type(t) => {
                checksum.update(2;
                t.update_checksum(checksum;
            }
            WitItem::Instance(inst) => {
                checksum.update(3;
                inst.update_checksum(checksum;
            }
        }
    }
}

impl Checksummable for WitFunction {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.params.update_checksum(checksum;
        self.results.update_checksum(checksum;
        checksum.update(if self.is_async { 1 } else { 0 };
    }
}

impl Checksummable for WitParam {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.ty.update_checksum(checksum;
    }
}

impl Checksummable for WitResult {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match &self.name {
            Some(name) => {
                checksum.update(1;
                name.update_checksum(checksum;
            }
            None => checksum.update(0),
        }
        self.ty.update_checksum(checksum;
    }
}

impl Checksummable for WitInstance {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.interface_name.update_checksum(checksum;
        self.args.update_checksum(checksum;
    }
}

impl Checksummable for WitInstanceArg {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.value.update_checksum(checksum;
    }
}

impl Checksummable for WitValue {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            WitValue::Type(t) => {
                checksum.update(0;
                t.update_checksum(checksum;
            }
            WitValue::Instance(i) => {
                checksum.update(1;
                i.update_checksum(checksum;
            }
        }
    }
}

impl Checksummable for WitTypeDef {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.ty.update_checksum(checksum;
        checksum.update(if self.is_resource { 1 } else { 0 };
    }
}

impl Checksummable for WitType {
    fn update_checksum(&self, checksum: &mut Checksum) {
        match self {
            WitType::Bool => checksum.update(0),
            WitType::U8 => checksum.update(1),
            WitType::U16 => checksum.update(2),
            WitType::U32 => checksum.update(3),
            WitType::U64 => checksum.update(4),
            WitType::S8 => checksum.update(5),
            WitType::S16 => checksum.update(6),
            WitType::S32 => checksum.update(7),
            WitType::S64 => checksum.update(8),
            WitType::F32 => checksum.update(9),
            WitType::F64 => checksum.update(10),
            WitType::Char => checksum.update(11),
            WitType::String => checksum.update(12),
            WitType::List(inner) => {
                checksum.update(13;
                inner.update_checksum(checksum;
            }
            WitType::Option(inner) => {
                checksum.update(14;
                inner.update_checksum(checksum;
            }
            WitType::Result { ok, err } => {
                checksum.update(15;
                match ok {
                    Some(t) => {
                        checksum.update(1;
                        t.as_ref().update_checksum(checksum;
                    }
                    None => checksum.update(0),
                }
                match err {
                    Some(t) => {
                        checksum.update(1;
                        t.as_ref().update_checksum(checksum;
                    }
                    None => checksum.update(0),
                }
            }
            WitType::Tuple(types) => {
                checksum.update(16;
                types.update_checksum(checksum;
            }
            WitType::Record(r) => {
                checksum.update(17;
                r.update_checksum(checksum;
            }
            WitType::Variant(v) => {
                checksum.update(18;
                v.update_checksum(checksum;
            }
            WitType::Enum(e) => {
                checksum.update(19;
                e.update_checksum(checksum;
            }
            WitType::Flags(f) => {
                checksum.update(20;
                f.update_checksum(checksum;
            }
            WitType::Own(name) => {
                checksum.update(21;
                name.update_checksum(checksum;
            }
            WitType::Borrow(name) => {
                checksum.update(22;
                name.update_checksum(checksum;
            }
            WitType::Named(name) => {
                checksum.update(23;
                name.update_checksum(checksum;
            }
            WitType::Stream(inner) => {
                checksum.update(24;
                inner.update_checksum(checksum;
            }
            WitType::Future(inner) => {
                checksum.update(25;
                inner.update_checksum(checksum;
            }
        }
    }
}

impl Checksummable for WitRecord {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.fields.update_checksum(checksum;
    }
}

impl Checksummable for WitRecordField {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.ty.update_checksum(checksum;
    }
}

impl Checksummable for WitVariant {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.cases.update_checksum(checksum;
    }
}

impl Checksummable for WitVariantCase {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        match &self.ty {
            Some(t) => {
                checksum.update(1;
                t.update_checksum(checksum;
            }
            None => checksum.update(0),
        }
    }
}

impl Checksummable for WitEnum {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.cases.update_checksum(checksum;
    }
}

impl Checksummable for WitFlags {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.flags.update_checksum(checksum;
    }
}

impl Checksummable for WitInterface {
    fn update_checksum(&self, checksum: &mut Checksum) {
        self.name.update_checksum(checksum;
        self.functions.update_checksum(checksum;
        self.types.update_checksum(checksum;
    }
}

// ===== ToBytes implementations =====

impl ToBytes for WitImport {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.item.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.item.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitExport {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.item.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.item.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitItem {
    fn serialized_size(&self) -> usize {
        1 + match self {
            WitItem::Function(f) => f.serialized_size(),
            WitItem::Interface(i) => i.serialized_size(),
            WitItem::Type(t) => t.serialized_size(),
            WitItem::Instance(inst) => inst.serialized_size(),
        }
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match self {
            WitItem::Function(f) => {
                writer.write_u8(0)?;
                f.to_bytes_with_provider(writer, provider)
            }
            WitItem::Interface(i) => {
                writer.write_u8(1)?;
                i.to_bytes_with_provider(writer, provider)
            }
            WitItem::Type(t) => {
                writer.write_u8(2)?;
                t.to_bytes_with_provider(writer, provider)
            }
            WitItem::Instance(inst) => {
                writer.write_u8(3)?;
                inst.to_bytes_with_provider(writer, provider)
            }
        }
    }
}

impl ToBytes for WitFunction {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + 
        self.params.serialized_size() + 
        self.results.serialized_size() + 
        1 // is_async
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.params.to_bytes_with_provider(writer, provider)?;
        self.results.to_bytes_with_provider(writer, provider)?;
        writer.write_u8(if self.is_async { 1 } else { 0 })
    }
}

impl ToBytes for WitParam {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.ty.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.ty.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitResult {
    fn serialized_size(&self) -> usize {
        1 + match &self.name {
            Some(name) => name.serialized_size(),
            None => 0,
        } + self.ty.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match &self.name {
            Some(name) => {
                writer.write_u8(1)?;
                name.to_bytes_with_provider(writer, provider)?;
            }
            None => writer.write_u8(0)?,
        }
        self.ty.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitInstance {
    fn serialized_size(&self) -> usize {
        self.interface_name.serialized_size() + self.args.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.interface_name.to_bytes_with_provider(writer, provider)?;
        self.args.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitInstanceArg {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.value.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.value.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitValue {
    fn serialized_size(&self) -> usize {
        1 + match self {
            WitValue::Type(t) => t.serialized_size(),
            WitValue::Instance(i) => i.serialized_size(),
        }
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match self {
            WitValue::Type(t) => {
                writer.write_u8(0)?;
                t.to_bytes_with_provider(writer, provider)
            }
            WitValue::Instance(i) => {
                writer.write_u8(1)?;
                i.to_bytes_with_provider(writer, provider)
            }
        }
    }
}

impl ToBytes for WitTypeDef {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.ty.serialized_size() + 1
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.ty.to_bytes_with_provider(writer, provider)?;
        writer.write_u8(if self.is_resource { 1 } else { 0 })
    }
}

impl ToBytes for WitType {
    fn serialized_size(&self) -> usize {
        1 + match self {
            WitType::Bool | WitType::U8 | WitType::U16 | WitType::U32 | WitType::U64 |
            WitType::S8 | WitType::S16 | WitType::S32 | WitType::S64 |
            WitType::F32 | WitType::F64 | WitType::Char | WitType::String => 0,
            WitType::List(inner) | WitType::Option(inner) | 
            WitType::Stream(inner) | WitType::Future(inner) => inner.serialized_size(),
            WitType::Result { ok, err } => {
                1 + ok.as_ref().map(|t| t.serialized_size()).unwrap_or(0) +
                1 + err.as_ref().map(|t| t.serialized_size()).unwrap_or(0)
            }
            WitType::Tuple(types) => types.serialized_size(),
            WitType::Record(r) => r.serialized_size(),
            WitType::Variant(v) => v.serialized_size(),
            WitType::Enum(e) => e.serialized_size(),
            WitType::Flags(f) => f.serialized_size(),
            WitType::Own(name) | WitType::Borrow(name) | WitType::Named(name) => name.serialized_size(),
        }
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        match self {
            WitType::Bool => writer.write_u8(0),
            WitType::U8 => writer.write_u8(1),
            WitType::U16 => writer.write_u8(2),
            WitType::U32 => writer.write_u8(3),
            WitType::U64 => writer.write_u8(4),
            WitType::S8 => writer.write_u8(5),
            WitType::S16 => writer.write_u8(6),
            WitType::S32 => writer.write_u8(7),
            WitType::S64 => writer.write_u8(8),
            WitType::F32 => writer.write_u8(9),
            WitType::F64 => writer.write_u8(10),
            WitType::Char => writer.write_u8(11),
            WitType::String => writer.write_u8(12),
            WitType::List(inner) => {
                writer.write_u8(13)?;
                inner.to_bytes_with_provider(writer, provider)
            }
            WitType::Option(inner) => {
                writer.write_u8(14)?;
                inner.to_bytes_with_provider(writer, provider)
            }
            WitType::Result { ok, err } => {
                writer.write_u8(15)?;
                match ok {
                    Some(t) => {
                        writer.write_u8(1)?;
                        t.as_ref().to_bytes_with_provider(writer, provider)?;
                    }
                    None => writer.write_u8(0)?,
                }
                match err {
                    Some(t) => {
                        writer.write_u8(1)?;
                        t.as_ref().to_bytes_with_provider(writer, provider)?;
                    }
                    None => writer.write_u8(0)?,
                }
                Ok(())
            }
            WitType::Tuple(types) => {
                writer.write_u8(16)?;
                types.to_bytes_with_provider(writer, provider)
            }
            WitType::Record(r) => {
                writer.write_u8(17)?;
                r.to_bytes_with_provider(writer, provider)
            }
            WitType::Variant(v) => {
                writer.write_u8(18)?;
                v.to_bytes_with_provider(writer, provider)
            }
            WitType::Enum(e) => {
                writer.write_u8(19)?;
                e.to_bytes_with_provider(writer, provider)
            }
            WitType::Flags(f) => {
                writer.write_u8(20)?;
                f.to_bytes_with_provider(writer, provider)
            }
            WitType::Own(name) => {
                writer.write_u8(21)?;
                name.to_bytes_with_provider(writer, provider)
            }
            WitType::Borrow(name) => {
                writer.write_u8(22)?;
                name.to_bytes_with_provider(writer, provider)
            }
            WitType::Named(name) => {
                writer.write_u8(23)?;
                name.to_bytes_with_provider(writer, provider)
            }
            WitType::Stream(inner) => {
                writer.write_u8(24)?;
                inner.to_bytes_with_provider(writer, provider)
            }
            WitType::Future(inner) => {
                writer.write_u8(25)?;
                inner.to_bytes_with_provider(writer, provider)
            }
        }
    }
}

impl ToBytes for WitRecord {
    fn serialized_size(&self) -> usize {
        self.fields.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.fields.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitRecordField {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + self.ty.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.ty.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitVariant {
    fn serialized_size(&self) -> usize {
        self.cases.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.cases.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitVariantCase {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + 1 + match &self.ty {
            Some(t) => t.serialized_size(),
            None => 0,
        }
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        match &self.ty {
            Some(t) => {
                writer.write_u8(1)?;
                t.to_bytes_with_provider(writer, provider)
            }
            None => writer.write_u8(0),
        }
    }
}

impl ToBytes for WitEnum {
    fn serialized_size(&self) -> usize {
        self.cases.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.cases.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitFlags {
    fn serialized_size(&self) -> usize {
        self.flags.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.flags.to_bytes_with_provider(writer, provider)
    }
}

impl ToBytes for WitInterface {
    fn serialized_size(&self) -> usize {
        self.name.serialized_size() + 
        self.functions.serialized_size() + 
        self.types.serialized_size()
    }

    fn to_bytes_with_provider<'a, P: MemoryProvider>(
        &self,
        writer: &mut WriteStream<'a>,
        provider: &P,
    ) -> Result<()> {
        self.name.to_bytes_with_provider(writer, provider)?;
        self.functions.to_bytes_with_provider(writer, provider)?;
        self.types.to_bytes_with_provider(writer, provider)
    }
}

// ===== FromBytes implementations =====

impl FromBytes for WitImport {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let item = WitItem::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, item })
    }
}

impl FromBytes for WitExport {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let item = WitItem::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, item })
    }
}

impl FromBytes for WitItem {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(WitItem::Function(WitFunction::from_bytes_with_provider(reader, provider)?)),
            1 => Ok(WitItem::Interface(WitInterface::from_bytes_with_provider(reader, provider)?)),
            2 => Ok(WitItem::Type(WitType::from_bytes_with_provider(reader, provider)?)),
            3 => Ok(WitItem::Instance(WitInstance::from_bytes_with_provider(reader, provider)?)),
            _ => Err(Error::runtime_execution_error("Invalid WitItem tag")),
        }
    }
}

impl FromBytes for WitFunction {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let params = BoundedVec::from_bytes_with_provider(reader, provider)?;
        let results = BoundedVec::from_bytes_with_provider(reader, provider)?;
        let is_async = reader.read_u8()? != 0;
        Ok(Self { name, params, results, is_async })
    }
}

impl FromBytes for WitParam {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedStringSmall::from_bytes_with_provider(reader, provider)?;
        let ty = WitType::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, ty })
    }
}

impl FromBytes for WitResult {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let has_name = reader.read_u8()? != 0;
        let name = if has_name {
            Some(WitBoundedStringSmall::from_bytes_with_provider(reader, provider)?)
        } else {
            None
        };
        let ty = WitType::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, ty })
    }
}

impl FromBytes for WitInstance {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let interface_name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let args = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { interface_name, args })
    }
}

impl FromBytes for WitInstanceArg {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedStringSmall::from_bytes_with_provider(reader, provider)?;
        let value = WitValue::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, value })
    }
}

impl FromBytes for WitValue {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(WitValue::Type(WitType::from_bytes_with_provider(reader, provider)?)),
            1 => Ok(WitValue::Instance(WitBoundedString::from_bytes_with_provider(reader, provider)?)),
            _ => Err(Error::new(ErrorCategory::Parse, wrt_error::codes::PARSE_ERROR, ")),
        }
    }
}

impl FromBytes for WitTypeDef {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let ty = WitType::from_bytes_with_provider(reader, provider)?;
        let is_resource = reader.read_u8()? != 0;
        Ok(Self { name, ty, is_resource })
    }
}

impl FromBytes for WitType {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let tag = reader.read_u8()?;
        match tag {
            0 => Ok(WitType::Bool),
            1 => Ok(WitType::U8),
            2 => Ok(WitType::U16),
            3 => Ok(WitType::U32),
            4 => Ok(WitType::U64),
            5 => Ok(WitType::S8),
            6 => Ok(WitType::S16),
            7 => Ok(WitType::S32),
            8 => Ok(WitType::S64),
            9 => Ok(WitType::F32),
            10 => Ok(WitType::F64),
            11 => Ok(WitType::Char),
            12 => Ok(WitType::String),
            13 => Ok(WitType::List(Box::new(WitType::from_bytes_with_provider(reader, provider)?))),
            14 => Ok(WitType::Option(Box::new(WitType::from_bytes_with_provider(reader, provider)?))),
            15 => {
                let has_ok = reader.read_u8()? != 0;
                let ok = if has_ok {
                    Some(Box::new(WitType::from_bytes_with_provider(reader, provider)?))
                } else {
                    None
                };
                let has_err = reader.read_u8()? != 0;
                let err = if has_err {
                    Some(Box::new(WitType::from_bytes_with_provider(reader, provider)?))
                } else {
                    None
                };
                Ok(WitType::Result { ok, err })
            }
            16 => Ok(WitType::Tuple(BoundedVec::from_bytes_with_provider(reader, provider)?)),
            17 => Ok(WitType::Record(WitRecord::from_bytes_with_provider(reader, provider)?)),
            18 => Ok(WitType::Variant(WitVariant::from_bytes_with_provider(reader, provider)?)),
            19 => Ok(WitType::Enum(WitEnum::from_bytes_with_provider(reader, provider)?)),
            20 => Ok(WitType::Flags(WitFlags::from_bytes_with_provider(reader, provider)?)),
            21 => Ok(WitType::Own(WitBoundedString::from_bytes_with_provider(reader, provider)?)),
            22 => Ok(WitType::Borrow(WitBoundedString::from_bytes_with_provider(reader, provider)?)),
            23 => Ok(WitType::Named(WitBoundedString::from_bytes_with_provider(reader, provider)?)),
            24 => Ok(WitType::Stream(Box::new(WitType::from_bytes_with_provider(reader, provider)?))),
            25 => Ok(WitType::Future(Box::new(WitType::from_bytes_with_provider(reader, provider)?))),
            _ => Err(Error::runtime_execution_error("Invalid WitType tag")),
        }
    }
}

impl FromBytes for WitRecord {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let fields = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { fields })
    }
}

impl FromBytes for WitRecordField {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedStringSmall::from_bytes_with_provider(reader, provider)?;
        let ty = WitType::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, ty })
    }
}

impl FromBytes for WitVariant {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let cases = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { cases })
    }
}

impl FromBytes for WitVariantCase {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedStringSmall::from_bytes_with_provider(reader, provider)?;
        let has_ty = reader.read_u8()? != 0;
        let ty = if has_ty {
            Some(WitType::from_bytes_with_provider(reader, provider)?)
        } else {
            None
        };
        Ok(Self { name, ty })
    }
}

impl FromBytes for WitEnum {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let cases = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { cases })
    }
}

impl FromBytes for WitFlags {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let flags = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { flags })
    }
}

impl FromBytes for WitInterface {
    fn from_bytes_with_provider<'a, P: MemoryProvider>(
        reader: &mut ReadStream<'a>,
        provider: &P,
    ) -> Result<Self> {
        let name = WitBoundedString::from_bytes_with_provider(reader, provider)?;
        let functions = BoundedVec::from_bytes_with_provider(reader, provider)?;
        let types = BoundedVec::from_bytes_with_provider(reader, provider)?;
        Ok(Self { name, functions, types })
    }
}