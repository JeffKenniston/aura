"""Cognition subpackage exposing Gemini Interactions API client, semantic cache, and context mitigation."""

from .cache import (
    CacheEntry,
    CacheLookupResult,
    SemanticCacheClient,
    compute_prompt_hash,
)
from .context import (
    DEFAULT_CONTEXT_LIMITS,
    DEFAULT_SATURATION_THRESHOLD,
    ContextManager,
    ContextSaturationError,
    ContextState,
    InfiniteSummarizationError,
    check_context_saturation,
    format_resumed_prompt,
)
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
    "DEFAULT_CONTEXT_LIMITS",
    "DEFAULT_SATURATION_THRESHOLD",
    "CacheEntry",
    "CacheLookupResult",
    "CognitiveEvent",
    "ContextManager",
    "ContextSaturationError",
    "ContextState",
    "EventType",
    "GeminiInteractionsClient",
    "InfiniteSummarizationError",
    "InteractionMetrics",
    "SecurityCredentialLeakError",
    "SemanticCacheClient",
    "SemanticPromptCache",
    "ToolArgumentValidationError",
    "ToolCall",
    "calculate_cost",
    "check_context_saturation",
    "compute_prompt_hash",
    "format_resumed_prompt",
    "sanitize_and_check_credentials",
    "validate_tool_arguments",
]
