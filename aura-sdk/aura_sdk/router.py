def route_cognitive_demand(prompt: str) -> dict:
    """
    Evaluates the cognitive demand of the prompt and returns the appropriate 
    model or agent routing configuration.
    
    Returns a dict with either:
      - {"model": "<model_name>"}
      - {"agent": "<agent_name>"}
    """
    prompt_lower = prompt.lower()
    
    # Tier 4: Deep Research (ungiven data collection and synthesis)
    deep_research_keywords = ["research", "collect data", "literature review", "competitive landscaping", "market analysis", "synthesize"]
    if any(kw in prompt_lower for kw in deep_research_keywords):
        return {"agent": "deep-research-preview"}
        
    # Tier 1: High Intelligence (complex multi-step reasoning, architectural planning)
    tier1_keywords = ["plan", "architecture", "complex", "design specification", "deep reasoning", "multi-step"]
    if any(kw in prompt_lower for kw in tier1_keywords) or len(prompt) > 2000:
        return {"model": "gemini-3.1-pro"}
        
    # Tier 3: High Throughput / Low Cost (validation, text classification)
    tier3_keywords = ["validate", "classify", "parse", "format", "check", "log analysis", "cost-sensitive"]
    if any(kw in prompt_lower for kw in tier3_keywords):
        return {"model": "gemini-3.5-flash-lite"}
        
    # Tier 2: Default Orchestration (broad enterprise workflows, tool orchestration)
    return {"model": "gemini-3.8-flash"}
