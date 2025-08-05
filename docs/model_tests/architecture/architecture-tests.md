# Instructions
There are 3 tests (below) to complete. Focus only one test at a time while ignoring the rest of the code base, docs, and other test files for now.

[BEGIN]
## Test 1 - System Architecture Design
Task: Design a scalable architecture for a real-time audio streaming service
```
Requirements:
- Support 100,000+ concurrent users
- Real-time audio transcoding (multiple formats)
- < 500ms latency for live streams
- Offline playback capability
- Global CDN distribution
- User authentication and subscription tiers
- Analytics and monitoring

Current Pain Points:
- Monolithic Node.js application hitting scaling limits
- Database bottlenecks during peak hours
- Inconsistent audio quality across regions
- No proper caching strategy
- Manual deployment process

Deliverables:
1. High-level architecture diagram (as ASCII or description)
2. Technology stack recommendations with justifications
3. Data flow design for audio processing pipeline
4. Scaling strategy (horizontal/vertical)
5. Failure recovery and redundancy approach
6. Migration plan from monolith to new architecture
```

## Test 2 - API Design Challenge
Task: Design a RESTful and/or GraphQL API for an audiobook platform
```
Domain Requirements:
- Users can browse, purchase, and stream audiobooks
- Authors can upload and manage their books
- Publishers can manage multiple authors
- Support for chapters, bookmarks, and playback position
- Social features: reviews, ratings, sharing
- Recommendation engine integration
- Multi-device synchronization

Design Requirements:
- Follow REST principles and/or GraphQL best practices
- Implement proper pagination strategies
- Design for offline-first mobile clients
- Include versioning strategy
- Define authentication/authorization approach
- Handle rate limiting and quotas
- Consider API gateway patterns

Deliverables:
1. API endpoint structure (REST routes or GraphQL schema)
2. Request/response examples for core operations
3. Error handling strategy and status codes
4. Authentication flow design
5. WebSocket/subscription design for real-time features
6. API documentation approach
```

## Test 3 - Database Architecture Challenge
Task: Design a database architecture for a podcast platform with social features
```
Functional Requirements:
- Store podcast metadata, episodes, and audio files
- User profiles, subscriptions, and listening history
- Social features: comments, likes, shares, follows
- Playlist creation and management
- Search across titles, descriptions, transcripts
- Analytics: play counts, completion rates, demographics
- Advertising: ad insertion points, campaign tracking

Scale Requirements:
- 50 million users
- 1 million podcasts
- 100 million episodes
- 1 billion play events per month
- < 100ms query response time
- 99.99% availability

Deliverables:
1. Choose database technologies (SQL/NoSQL/hybrid) with justification
2. Design core data models and relationships
3. Define sharding/partitioning strategy
4. Design indexing strategy for search and queries
5. Create data consistency approach (eventual/strong)
6. Design backup and disaster recovery plan
7. Define caching layers and strategies
```
[END]