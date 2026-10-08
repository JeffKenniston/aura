"""Cognition subpackage exposing Gemini Interactions API client and data models."""

from .gemini import (
    CognitiveEvent,
    EventType,
    GeminiInteractionsClient,
    InteractionMetrics,
    SecurityCredentialLeakError,
    SemanticPromptCache,
    ToolArgumentValidationError,
    ToolCall,
    calculate_cost,
    sanitize_and_check_credentials,
    validate_tool_arguments,
)

__all__ = [
    "CognitiveEvent",
    "EventType",
    "GeminiInteractionsClient",
    "InteractionMetrics",
    "SecurityCredentialLeakError",
    "SemanticPromptCache",
    "ToolArgumentValidationError",
    "ToolCall",
    "calculate_cost",
    "sanitize_and_check_credentials",
    "validate_tool_arguments",
]
