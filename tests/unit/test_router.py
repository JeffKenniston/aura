import pytest
from aura_sdk.router import route_cognitive_demand

def test_route_cognitive_demand():
    # Tier 4
    assert route_cognitive_demand("Please research the market analysis") == {"agent": "deep-research-preview"}
    
    # Tier 1
    assert route_cognitive_demand("We need an architecture plan for this complex multi-step thing") == {"model": "gemini-3.1-pro"}
    
    # Tier 3
    assert route_cognitive_demand("Please validate and parse this log analysis") == {"model": "gemini-3.5-flash-lite"}
    
    # Tier 2 (Default)
    assert route_cognitive_demand("Write a simple script") == {"model": "gemini-3.8-flash"}
