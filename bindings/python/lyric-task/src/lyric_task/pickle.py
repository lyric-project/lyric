"""Custom serialization module.

This module provides a custom serialization mechanism that can be used in webassembly environments.
"""

import base64
import inspect
import json
import logging
import textwrap
import types
from dataclasses import dataclass
from functools import wraps
from typing import Any, Dict, Optional

import cloudpickle

logger = logging.getLogger(__name__)


@dataclass
class SerializationResult:
    """Serialization result data class."""

    cloudpickle: Optional[str] = None
    custom: Optional[str] = None
    cloudpickle_error: Optional[str] = None
    custom_error: Optional[str] = None


def handle_serialization_errors(func):
    """Decorator to handle serialization errors."""

    @wraps(func)
    def wrapper(*args, **kwargs):
        try:
            return func(*args, **kwargs)
        except Exception as e:
            print(f"Serialization error in {func.__name__}: {str(e)}")
            raise SerializationError(f"Failed to serialize: {str(e)}")

    return wrapper


class SerializationError(Exception):
    """Custom serialization exception."""

    def __init__(self, message: str):
        super().__init__(message)


class Serializer:
    """Serializer class."""

    @staticmethod
    @handle_serialization_errors
    def serialize_to_binary(item: Any) -> bytes:
        """Serialize object to binary format.

        Args:
            item: The object to serialize.

        Returns:
            bytes: The serialized object.

        Raises:
            SerializationError: When serialization fails.
        """
        result = SerializationResult()
        try:
            # First try cloudpickle
            cloudpickle_result = cloudpickle.dumps(item)
            result.cloudpickle = base64.b64encode(cloudpickle_result).decode("utf-8")
        except Exception as e:
            result.cloudpickle_error = str(e)

        try:
            # Custom serialization
            if isinstance(item, types.FunctionType):
                custom_result = Serializer._serialize_function(item)
            else:
                custom_result = Serializer._serialize_object(item)
            result.custom = base64.b64encode(custom_result).decode("utf-8")
        except Exception as e:
            result.custom_error = str(e)

        # Make sure at least one serialization method succeeded
        if result.cloudpickle is None and result.custom is None:
            raise SerializationError("Both serialization methods failed")

        return json.dumps(result.__dict__).encode("utf-8")

    @staticmethod
    @handle_serialization_errors
    def _serialize_function(func: types.FunctionType) -> bytes:
        """Serialize function."""
        func_data = {
            "type": "function",
            "name": func.__name__,
            "code": inspect.getsource(func),
            "globals": {
                k: repr(v)
                for k, v in func.__globals__.items()
                if k in func.__code__.co_names
            },
        }
        return json.dumps(func_data).encode("utf-8")

    @staticmethod
    @handle_serialization_errors
    def _serialize_object(obj: Any) -> bytes:
        """Serialize object."""
        class_def = inspect.getsource(obj.__class__)
        class_def = textwrap.dedent(class_def)
        obj_data = {
            "type": "object",
            "class_def": class_def,
            "class_name": obj.__class__.__name__,
            "attributes": {k: repr(v) for k, v in obj.__dict__.items()},
        }
        return json.dumps(obj_data).encode("utf-8")


class Deserializer:
    """Deserializer class."""

    @staticmethod
    @handle_serialization_errors
    def deserialize_from_binary(binary_data: bytes) -> Any:
        """Deserialize object from binary data.

        Args:
            binary_data: The binary data to deserialize.

        Returns:
            Any: The deserialized object.

        Raises:
            ValueError: When deserialization fails.
        """
        data = json.loads(binary_data.decode("utf-8"))

        # Try to deserialize using cloudpickle
        if "cloudpickle" in data and data["cloudpickle"] is not None:
            try:
                return cloudpickle.loads(base64.b64decode(data["cloudpickle"]))
            except Exception as e:
                logger.error(f"Cloudpickle deserialization failed: {e}")

        # If cloudpickle failed or is not present, try custom deserialization
        if "custom" in data and data["custom"] is not None:
            try:
                custom_data = json.loads(base64.b64decode(data["custom"]))
                if custom_data["type"] == "function":
                    return Deserializer._deserialize_function(custom_data)
                else:
                    return Deserializer._deserialize_object(custom_data)
            except Exception as e:
                logger.error(f"Custom deserialization failed: {e}")

        raise ValueError("Unable to deserialize data")

    @staticmethod
    @handle_serialization_errors
    def _deserialize_function(func_data: Dict) -> types.FunctionType:
        """Deserialize function."""
        globals_dict = {}
        for name, value in func_data["globals"].items():
            try:
                globals_dict[name] = eval(value, {}, {})
            except:
                globals_dict[name] = value

        # Deserialize function with exec
        exec(func_data["code"], globals_dict)
        return globals_dict[func_data["name"]]

    @staticmethod
    @handle_serialization_errors
    def _deserialize_object(obj_data: Dict) -> Any:
        """Deserialize object."""

        namespace = {}
        exec(obj_data["class_def"], namespace)
        cls = namespace[obj_data["class_name"]]
        obj = cls.__new__(cls)

        for attr, value in obj_data["attributes"].items():
            try:
                setattr(obj, attr, eval(value, {}, {}))
            except:
                setattr(obj, attr, value)

        return obj


# Keep the original interface for compatibility
loads = Deserializer.deserialize_from_binary
dumps = Serializer.serialize_to_binary
