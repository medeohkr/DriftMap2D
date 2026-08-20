import numpy as np

def step(x):
    if x < 0:  # Low noise
        return np.random.normal(0, 1)  # Small steps (σ=1)
    else:      # High noise
        return np.random.normal(0, 5)  # Large steps (σ=5)

# Run 100,000 particles for 1000 steps
positions = np.zeros(10000)
for _ in range(1000):
    positions += [step(x) for x in positions]

# What's the average position?
print(f"Mean position: {np.mean(positions)}")  
# Output: Mean position: ~??? (positive!)