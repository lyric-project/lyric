import os
import uuid
from abc import ABC, abstractmethod
from enum import Enum
from typing import Dict, List, Optional, Type, TypeVar, Union

from .pickle import Deserializer, Serializer

T = TypeVar("T", bound="BaseEnum")


class BaseEnum(Enum):
    @classmethod
    def from_string(cls: Type[T], name: str) -> Optional[T]:
        """Get enum member by string name (case-insensitive)"""
        try:
            return cls[name.upper()]
        except KeyError:
            return None

    @classmethod
    def from_int(cls: Type[T], value: int) -> Optional[T]:
        """Get enum member by integer value"""
        try:
            return cls(value)
        except ValueError:
            return None

    @classmethod
    def get_all_names(cls) -> List[str]:
        """Get all enum member names"""
        return [member.name for member in cls]

    @classmethod
    def get_all_values(cls) -> List[int]:
        """Get all enum values"""
        return [member.value for member in cls]

    def to_dict(self) -> Dict[str, Union[str, int]]:
        """Convert enum member to dictionary"""
        return {"name": self.name, "value": self.value}

    @classmethod
    def contains_name(cls, name: str) -> bool:
        """Check if enum contains the given name"""
        return name.upper() in cls.__members__

    @classmethod
    def contains_value(cls, value: int) -> bool:
        """Check if enum contains the given value"""
        return any(member.value == value for member in cls)

    @classmethod
    def get_pairs(cls) -> List[tuple]:
        """Get all (name, value) pairs"""
        return [(member.name, member.value) for member in cls]

    def next(self: T) -> Optional[T]:
        """Get next enum member if exists"""
        members = list(self.__class__)
        index = members.index(self)
        if index < len(members) - 1:
            return members[index + 1]
        return None

    @classmethod
    def parse(cls: Type[T], value: Union[T, int, str, None]) -> Optional[T]:
        """
        Parse various input types to enum member.

        Args:
            value: Can be one of:
                - None: returns None
                - Enum member: returns the member if it's of the same type
                - int: converts to enum member by value
                - str: converts to enum member by name (case-insensitive)

        Returns:
            Optional[T]: The corresponding enum member or None if conversion fails
        """
        if value is None:
            return None

        # If already correct enum type, return it
        if isinstance(value, cls):
            return value

        # If it's an integer, try converting by value
        if isinstance(value, int):
            return cls.from_int(value)

        # If it's a string, try converting by name
        if isinstance(value, str):
            return cls.from_string(value)

        return None


class Language(BaseEnum):
    PYTHON = 0
    JAVASCRIPT = 1
    SHELL = 2
    RUST = 3


LanguageType = Union[Language, int, str]


class DataFormat(BaseEnum):
    RAW = 0
    PICKLE = 1
    MSGPACK = 2
    PROTOBUF = 3

    def is_binary(self) -> bool:
        """Check if the format is binary"""
        return self != self.RAW

    @classmethod
    def get_binary_formats(cls) -> List["DataFormat"]:
        """Get all binary format types"""
        return [fmt for fmt in cls if fmt != cls.RAW]


class DataObject:

    def __init__(self, object_id: str, format: int, data: bytes):
        self.object_id = object_id
        self.format = format
        self.data = data


class ExecutionUnit:
    def __init__(self, unit_id: str, language: int, code: DataObject):
        self.unit_id = unit_id
        self.language = language
        self.code = code


class BaseTaskSpec(ABC):

    @abstractmethod
    def to_execution_unit(self) -> ExecutionUnit:
        """Convert task to execution unit"""

    @abstractmethod
    def run(self):
        raise NotImplementedError

    @property
    def streaming_result(self):
        return False

    def __call__(self):
        return self.run()


class ExecutableTaskSpec(BaseTaskSpec, ABC):

    def to_execution_unit(self) -> ExecutionUnit:
        code_data_format = DataFormat.PICKLE
        code_data = Serializer.serialize_to_binary(self)
        code_object = DataObject(
            object_id=str(uuid.uuid4()), format=code_data_format.value, data=code_data
        )
        return ExecutionUnit(
            unit_id=str(uuid.uuid4()), language=Language.PYTHON.value, code=code_object
        )


class FunctionTaskSpec(ExecutableTaskSpec):
    def __init__(self, func: callable):
        self.func = func

    def run(self):
        return self.func()


class WasmTaskSpec(BaseTaskSpec):
    def __init__(self, wasm_path: str, lang: LanguageType = Language.PYTHON):
        if not os.path.exists(wasm_path):
            raise ValueError(f"Wasm file not found: {wasm_path}")
        self.wasm_path = wasm_path
        lang = Language.parse(lang)
        if not lang:
            raise ValueError(f"Invalid language: {lang}")
        self.lang = lang

    def to_execution_unit(self) -> ExecutionUnit:
        code_data_format = DataFormat.RAW
        code_data = self.wasm_path.encode("utf-8")
        code_object = DataObject(
            object_id=str(uuid.uuid4()), format=code_data_format.value, data=code_data
        )
        return ExecutionUnit(
            unit_id=str(uuid.uuid4()), language=self.lang.value, code=code_object
        )

    def run(self):
        raise NotImplementedError("WasmTaskSpec is not executable")
