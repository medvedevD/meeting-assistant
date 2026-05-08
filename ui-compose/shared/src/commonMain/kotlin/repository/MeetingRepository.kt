package repository

import domain.JobProgress
import domain.Meeting
import domain.Protocol

interface MeetingRepository {
    suspend fun list(): List<Meeting>
    suspend fun protocolLoad(meetingId: String): Protocol?
    suspend fun protocolGenerate(meetingId: String, templateName: String?): Protocol
    suspend fun jobSubmit(audioPath: String, name: String?): JobProgress
    suspend fun jobStatus(jobId: String): JobProgress?
}
