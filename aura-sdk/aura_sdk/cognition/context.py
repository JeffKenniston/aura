"""
Context Saturation Mitigation Engine for Aura SDK.
Implements GAP-006 and GAP-008.

Monitors interaction token chain. At 80% context window saturation:
1. Pauses the main task.
2. Dispatches a summarization prompt using Flash-Lite (configurable).
3. Seeds a new Interaction ID with the summarized background.
4. Enforces a hard limit on consecutive summarization loops to prevent infinite recursion.
"""

from __future__ import annotations

import json
import logging
import time
import uuid
from dataclasses import dataclass, field
from typing import Any

logger = logging.getLogger("aura_sdk.cognition.context")

DEFAULT_CONTEXT_LIMITS: dict[str, int] = {
    "gemini-3.8-flash": 1_000_000,
    "gemini-3.1-pro": 2_000_000,
    "gemini-3.5-flash-lite": 1_000_000,
    "deep-research-preview": 1_000_000,
}

DEFAULT_MAX_CONTEXT_TOKENS: int = 1_000_000
DEFAULT_SATURATION_THRESHOLD: float = 0.80  # 80% context saturation threshold (GAP-006)
DEFAULT_MAX_CONSECUTIVE_SUMMARIZATIONS: int = 3  # Hard limit on recursive loops (GAP-008)

DEFAULT_SUMMARIZATION_PROMPT = (
    "System Context Compression: Summarize the entire conversation history, "
    "architectural decisions, and current state into a dense context document."
)


class ContextSaturationError(Exception):
    """Base error for context saturation conditions."""


class InfiniteSummarizationError(ContextSaturationError):
    """Raised when consecutive summarization loops exceed the hard limit (GAP-008)."""


@dataclass
class ContextState:
    """Tracks token context and summarization lifecycle state."""

    current_tokens: int = 0
    max_tokens: int = DEFAULT_MAX_CONTEXT_TOKENS
    saturation_threshold: float = DEFAULT_SATURATION_THRESHOLD
    consecutive_summarizations: int = 0
    max_consecutive_summarizations: int = DEFAULT_MAX_CONSECUTIVE_SUMMARIZATIONS
    current_interaction_id: str | None = None
    summary_history: list[str] = field(default_factory=list)
    is_paused: bool = False
    last_summarized_at: float | None = None

    @property
    def saturation_ratio(self) -> float:
        """Current ratio of context window consumption."""
        if self.max_tokens <= 0:
            return 0.0
        return self.current_tokens / float(self.max_tokens)

    def is_saturated(self) -> bool:
        """Returns True if context window usage >= saturation threshold (80%)."""
        return self.saturation_ratio >= self.saturation_threshold


def check_context_saturation(
    current_tokens: int,
    max_tokens: int = DEFAULT_MAX_CONTEXT_TOKENS,
    threshold: float = DEFAULT_SATURATION_THRESHOLD,
) -> bool:
    """Helper function to check if token count is at or above saturation threshold."""
    if max_tokens <= 0:
        return False
    return (current_tokens / float(max_tokens)) >= threshold


def format_resumed_prompt(summary: str, next_instruction: str) -> str:
    """Formats the distilled prompt seeded with the summarized background context."""
    return f"Previous context summary: {summary}\n\nNext instruction: {next_instruction}"


class ContextManager:
    """
    Manages context window saturation monitoring and automated summarization loops.
    Implements GAP-006 (80% saturation pause and Flash-Lite summarization)
    and GAP-008 (infinite recursive summarization prevention).
    """

    def __init__(
        self,
        max_tokens: int = DEFAULT_MAX_CONTEXT_TOKENS,
        saturation_threshold: float = DEFAULT_SATURATION_THRESHOLD,
        summarization_model: str = "gemini-3.5-flash-lite",
        max_consecutive_summarizations: int = DEFAULT_MAX_CONSECUTIVE_SUMMARIZATIONS,
        summarization_prompt: str = DEFAULT_SUMMARIZATION_PROMPT,
        bus_client: Any | None = None,
        tenant_id: str = "default",
    ):
        self.summarization_model = summarization_model
        self.summarization_prompt = summarization_prompt
        self.bus_client = bus_client
        self.tenant_id = tenant_id

        self.state = ContextState(
            max_tokens=max_tokens,
            saturation_threshold=saturation_threshold,
            max_consecutive_summarizations=max_consecutive_summarizations,
        )

    def set_model_context_limit(self, model: str) -> None:
        """Updates max tokens based on known model limits."""
        if model in DEFAULT_CONTEXT_LIMITS:
            self.state.max_tokens = DEFAULT_CONTEXT_LIMITS[model]

    def record_tokens(
        self,
        tokens: int,
        interaction_id: str | None = None,
        model: str | None = None,
    ) -> bool:
        """
        Updates token count and returns True if 80% saturation is reached.
        """
        if model:
            self.set_model_context_limit(model)

        self.state.current_tokens = tokens
        if interaction_id:
            self.state.current_interaction_id = interaction_id

        saturated = self.state.is_saturated()
        if saturated:
            logger.warning(
                f"Context saturation threshold ({self.state.saturation_threshold * 100:.0f}%) reached: "
                f"{self.state.current_tokens}/{self.state.max_tokens} tokens ({self.state.saturation_ratio * 100:.1f}%)."
            )
        return saturated

    def is_saturated(
        self,
        tokens: int | None = None,
        max_tokens: int | None = None,
    ) -> bool:
        """Evaluates whether saturation threshold is reached."""
        if tokens is not None:
            max_t = max_tokens or self.state.max_tokens
            return check_context_saturation(tokens, max_t, self.state.saturation_threshold)
        return self.state.is_saturated()

    def pause_task(self) -> None:
        """Pauses the main task execution for context compression."""
        self.state.is_paused = True
        logger.info("Main task PAUSED for context compression.")
        if self.bus_client and hasattr(self.bus_client, "publish"):
            payload = {
                "action": "pause",
                "reason": "context_saturation",
                "tokens": self.state.current_tokens,
                "max_tokens": self.state.max_tokens,
                "timestamp": time.time(),
            }
            try:
                self.bus_client.publish(
                    f"aura/{self.tenant_id}/context/pause",
                    json.dumps(payload),
                )
            except Exception as e:
                logger.warning(f"Failed to publish pause event: {e}")

    def resume_task(self) -> None:
        """Resumes the main task execution with the distilled background."""
        self.state.is_paused = False
        logger.info("Main task RESUMED.")
        if self.bus_client and hasattr(self.bus_client, "publish"):
            payload = {
                "action": "resume",
                "timestamp": time.time(),
            }
            try:
                self.bus_client.publish(
                    f"aura/{self.tenant_id}/context/resume",
                    json.dumps(payload),
                )
            except Exception as e:
                logger.warning(f"Failed to publish resume event: {e}")

    def reset_consecutive_loops(self) -> None:
        """Resets the consecutive summarization counter upon normal turn completion."""
        self.state.consecutive_summarizations = 0

    async def summarize_and_seed(
        self,
        client: Any,
        current_prompt: str,
        interaction_id: str | None = None,
        summarization_model: str | None = None,
    ) -> tuple[str, str]:
        """
        Executes the context saturation mitigation workflow (GAP-006, GAP-008):
        1. Checks hard limit on consecutive summarizations to prevent infinite loops.
        2. Pauses the main task.
        3. Dispatches summarization prompt using Flash-Lite.
        4. Seeds a new Interaction ID with the distilled context.
        5. Resumes execution and returns (distilled_prompt, new_interaction_id).
        """
        # Hard limit guard against infinite recursive summarization (GAP-008)
        if self.state.consecutive_summarizations >= self.state.max_consecutive_summarizations:
            self.state.is_paused = False
            raise InfiniteSummarizationError(
                f"Context saturation loop detected! Hard limit of {self.state.max_consecutive_summarizations} "
                f"consecutive summarizations exceeded. Aborting to prevent infinite spend."
            )

        # 1. Pause task
        self.pause_task()

        active_interaction_id = interaction_id or self.state.current_interaction_id
        model_tier = summarization_model or self.summarization_model

        logger.info(
            f"DISPATCHING SUMMARIZATION: model={model_tier}, "
            f"previous_interaction_id={active_interaction_id}, loop_count={self.state.consecutive_summarizations + 1}"
        )

        summary_kwargs: dict[str, Any] = {
            "input": self.summarization_prompt,
            "model": model_tier,
            "stream": True,
        }
        if active_interaction_id:
            summary_kwargs["previous_interaction_id"] = active_interaction_id

        summary_text = ""
        try:
            if hasattr(client, "aio") and hasattr(client.aio, "interactions"):
                response_stream = await client.aio.interactions.create(**summary_kwargs)
                async for ev in response_stream:
                    ev_type = getattr(ev, "event_type", "")
                    delta = getattr(ev, "delta", None)
                    if ev_type == "step.delta" and delta:
                        d_type = getattr(delta, "type", "")
                        if d_type == "text":
                            summary_text += getattr(delta, "text", "")
            elif hasattr(client, "stream_interaction"):
                async for ev in client.stream_interaction(
                    prompt=self.summarization_prompt,
                    model=model_tier,
                ):
                    if ev.event_type == "cog.text_delta" and ev.delta_text:
                        summary_text += ev.delta_text
            else:
                summary_text = "Summary of conversation history and system state."
        except Exception as e:
            logger.error(f"Failed to generate context summary: {e}")
            self.resume_task()
            raise

        if not summary_text:
            summary_text = "Dense summary of previous interactions."

        # 2. Seed a new Interaction ID
        new_seed_id = f"seed-{uuid.uuid4().hex[:12]}"
        self.state.current_interaction_id = new_seed_id
        self.state.summary_history.append(summary_text)
        self.state.last_summarized_at = time.time()
        self.state.consecutive_summarizations += 1

        # Reset tokens to estimated size of the summary
        self.state.current_tokens = max(100, len(summary_text) // 4)

        # 3. Format distilled prompt with background summary
        distilled_prompt = format_resumed_prompt(summary_text, current_prompt)

        # 4. Resume execution
        self.resume_task()

        logger.info(
            f"Context compression complete. Seeded new Interaction ID: {new_seed_id}. "
            f"Consecutive loops: {self.state.consecutive_summarizations}"
        )

        return distilled_prompt, new_seed_id
