# Runed Opportunity Audit — Output Format

Present findings using this structure.

## 🔍 Runed Opportunity Analysis

### Found Opportunities

#### 🔴 HIGH PRIORITY - [Pattern Name]

**File**: `path/to/file.svelte:10-35`  
**Current Pattern**: [Brief description]  
**Runed Utility**: `UtilityName`

**Impact**:

- Lines: ~X → ~Y (**-Z%**)
- State variables: X → Y
- Manual cleanup: Yes → No
- Memory leak risk: Present → Eliminated or Reduced

**Current Code**:

```ts
// Real snippet from the codebase — 10–15 lines with enough context
```

**Suggested Fix**:

```ts
import { UtilityName } from 'runed';

const source = $state('');
const derivedValue = UtilityName(source, {
	// Concrete options matching this code path
});

$effect(() => {
	// Preserve existing behavior; remove manual teardown if Runed handles it
});
```

**Notes**:

- Why this utility fits
- Behavior caveats or migration constraints
- Follow-up tests to add or update

#### 🟡 MEDIUM PRIORITY - [Pattern Name]

**File**: `path/to/file.svelte:40-78`  
**Current Pattern**: [Brief description]  
**Runed Utility**: `UtilityName`

**Impact**:

- Lines: ~X → ~Y (**-Z%**)
- State variables: X → Y
- Manual cleanup: Yes → No
- Memory leak risk: Present → Reduced

**Current Code**:

```ts
// Real snippet from the codebase
```

**Suggested Fix**:

```ts
// Focused replacement using the Runed utility
```

**Notes**:

- Key implementation details
- Potential regressions to watch for

#### 🟢 LOW PRIORITY - [Pattern Name]

**File**: `path/to/file.svelte:90-120`  
**Current Pattern**: [Brief description]  
**Runed Utility**: `UtilityName`

**Impact**:

- Lines: ~X → ~Y (**-Z%**)
- State variables: X → Y
- Manual cleanup: Yes → No
- Memory leak risk: Low → Lower

**Current Code**:

```ts
// Real snippet from the codebase
```

**Suggested Fix**:

```ts
// Replacement snippet
```

**Notes**:

- Why this is lower priority
- When to schedule this refactor

### Summary

- High priority opportunities: [count]
- Medium priority opportunities: [count]
- Low priority opportunities: [count]

### Recommended First Implementation

- **Start with**: [Selected high-priority opportunity]
- **Why first**: [Highest impact with lowest migration risk]
- **Estimated effort**: Small | Medium | Large

### If No Opportunities Are Found

State explicitly: **"No meaningful Runed opportunities found in this codebase at this time."**

Include a brief rationale and stop.
